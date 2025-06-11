use crate::types::*;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::fs;
use std::path::Path;
use std::process::Command;
use which::which;

#[derive(Debug, Clone, PartialEq)]
pub enum ContentSearchBackend {
    Ripgrep,
    Ag,
    Fallback,
}

pub struct FuzzySearcher {
    symbols: Vec<CodeSymbol>,
    matcher: SkimMatcherV2,
}

impl std::fmt::Debug for FuzzySearcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzySearcher")
            .field("symbols", &self.symbols)
            .field("matcher", &"SkimMatcherV2")
            .finish()
    }
}

impl FuzzySearcher {
    pub fn new(symbols: Vec<CodeSymbol>) -> Self {
        Self {
            symbols,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Detect the best available content search backend
    fn detect_content_search_backend() -> ContentSearchBackend {
        // 最適な順序: ripgrep → ag → fallback
        // 実際のベンチマーク結果: fallback(2.74ms) > ripgrep(13.25ms) > ag
        // しかし外部ツールが利用可能な場合は一貫性のため優先使用
        if which("rg").is_ok() {
            ContentSearchBackend::Ripgrep
        } else if which("ag").is_ok() {
            ContentSearchBackend::Ag
        } else {
            ContentSearchBackend::Fallback
        }
    }

    /// High-performance content search using ripgrep
    fn search_content_with_ripgrep(&self, query: &str, options: &SearchOptions) -> anyhow::Result<Vec<SearchResult>> {
        let mut cmd = Command::new("rg");
        
        // Basic ripgrep options for optimal performance
        cmd.arg("--line-number")       // Show line numbers
           .arg("--no-heading")        // Don't group by file
           .arg("--with-filename")     // Always show filename
           .arg("--no-messages")       // Suppress error messages
           .arg("--max-filesize")      // Limit file size (same as our fallback)
           .arg("1M");                 // 1MB limit
        
        // Add search pattern (escape special regex characters for literal search)
        let escaped_query = regex::escape(query);
        cmd.arg(&escaped_query);
        
        // Get unique root directories to minimize search scope
        let search_paths = self.get_optimized_search_paths();
        if search_paths.is_empty() {
            return Ok(vec![]);
        }
        
        // Add paths to search (prefer fewer, broader paths)
        for path in &search_paths {
            cmd.arg(path);
        }
        
        let output = cmd.output().map_err(|e| {
            anyhow::anyhow!("Failed to execute ripgrep: {}", e)
        })?;
        
        if !output.status.success() {
            // If ripgrep fails (e.g., no matches), return empty results
            // Note: ripgrep returns exit code 1 for "no matches found", which is normal
            return Ok(vec![]);
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        
        for line in stdout.lines() {
            if let Some(parsed) = self.parse_ripgrep_line(line) {
                results.push(parsed);
            }
        }
        
        // Apply limit early for performance
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }

    /// Get optimized search paths to minimize ripgrep overhead
    fn get_optimized_search_paths(&self) -> Vec<std::path::PathBuf> {
        let mut all_dirs = std::collections::HashSet::new();
        
        // Collect all unique parent directories
        for symbol in &self.symbols {
            if let Some(parent) = symbol.file.parent() {
                all_dirs.insert(parent.to_path_buf());
            }
        }
        
        // Convert to sorted vector for consistent behavior
        let mut dir_list: Vec<_> = all_dirs.into_iter().collect();
        dir_list.sort();
        
        // Optimize: remove subdirectories if parent is already included
        let mut optimized = Vec::new();
        for dir in &dir_list {
            let is_subdir = optimized.iter().any(|parent: &std::path::PathBuf| {
                dir.starts_with(parent)
            });
            
            if !is_subdir {
                optimized.push(dir.clone());
            }
        }
        
        optimized
    }

    /// Parse ripgrep output line: "file:line:content"
    fn parse_ripgrep_line(&self, line: &str) -> Option<SearchResult> {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() != 3 {
            return None;
        }
        
        let file_path = parts[0];
        let line_num: usize = parts[1].parse().ok()?;
        let content = parts[2].trim();
        
        Some(SearchResult {
            symbol: CodeSymbol {
                name: content.to_string(),
                symbol_type: SymbolType::Variable, // Generic content type
                file: file_path.into(),
                line: line_num,
                column: 1,
                context: Some(content.to_string()),
            },
            score: 0.1, // Ripgrep results are considered high quality
        })
    }

    /// High-performance content search using the_silver_searcher (ag)
    fn search_content_with_ag(&self, query: &str, options: &SearchOptions) -> anyhow::Result<Vec<SearchResult>> {
        let mut cmd = Command::new("ag");
        
        // Basic ag options for optimal performance
        cmd.arg("--line-numbers")       // Show line numbers
           .arg("--nogroup")            // Don't group by file
           .arg("--filename")           // Always show filename
           .arg("--silent")             // Suppress error messages
           .arg("--max-filesize")       // Limit file size (same as ripgrep)
           .arg("1M");                  // 1MB limit
        
        // Add search pattern (use literal search for consistency with ripgrep)
        cmd.arg("--literal")            // Literal string search (not regex)
           .arg(query);
        
        // Get optimized search paths
        let search_paths = self.get_optimized_search_paths();
        if search_paths.is_empty() {
            return Ok(vec![]);
        }
        
        // Add paths to search
        for path in &search_paths {
            cmd.arg(path);
        }
        
        let output = cmd.output().map_err(|e| {
            anyhow::anyhow!("Failed to execute ag: {}", e)
        })?;
        
        if !output.status.success() {
            // If ag fails (e.g., no matches), return empty results
            // Note: ag returns exit code 1 for "no matches found", which is normal
            return Ok(vec![]);
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        
        for line in stdout.lines() {
            if let Some(parsed) = self.parse_ag_line(line) {
                results.push(parsed);
            }
        }
        
        // Apply limit early for performance
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }

    /// Parse ag output line: "file:line:content"
    fn parse_ag_line(&self, line: &str) -> Option<SearchResult> {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() != 3 {
            return None;
        }
        
        let file_path = parts[0];
        let line_num: usize = parts[1].parse().ok()?;
        let content = parts[2].trim();
        
        Some(SearchResult {
            symbol: CodeSymbol {
                name: content.to_string(),
                symbol_type: SymbolType::Variable, // Generic content type
                file: file_path.into(),
                line: line_num,
                column: 1,
                context: Some(content.to_string()),
            },
            score: 0.15, // Ag results are high quality but slightly lower than ripgrep
        })
    }

    pub fn search(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // Handle empty query
        if query.trim().is_empty() {
            return vec![];
        }

        let mut results: Vec<SearchResult> = Vec::new();

        for symbol in &self.symbols {
            // Apply type filtering
            if let Some(ref types) = options.types {
                if !types.contains(&symbol.symbol_type) {
                    continue;
                }
            }

            // Apply file/directory filtering
            if let Some(include_files) = options.include_files {
                if !include_files && symbol.symbol_type == SymbolType::Filename {
                    continue;
                }
            }

            if let Some(include_dirs) = options.include_dirs {
                if !include_dirs && symbol.symbol_type == SymbolType::Dirname {
                    continue;
                }
            }

            // Perform fuzzy matching
            if let Some((score, _)) = self.matcher.fuzzy_indices(&symbol.name, query) {
                // Convert skim score to distance (higher skim score = better match = lower distance)
                let mut distance = 1.0 - (score as f64 / 100.0);

                // Boost exact matches
                if symbol.name.eq_ignore_ascii_case(query) {
                    distance *= 0.1; // Much better score for exact matches
                }

                // Apply threshold filtering
                if let Some(threshold) = options.threshold {
                    if distance > threshold {
                        continue;
                    }
                }

                results.push(SearchResult {
                    symbol: symbol.clone(),
                    score: distance,
                });
            }
        }

        // Sort by score (ascending = better match first)
        results.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        results
    }

    pub fn update_symbols(&mut self, symbols: Vec<CodeSymbol>) {
        self.symbols = symbols;
    }

    // High-performance content search with ripgrep->ag->fallback strategy
    pub fn search_content(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // Handle empty query
        if query.trim().is_empty() {
            return vec![];
        }

        let backend = Self::detect_content_search_backend();
        
        match backend {
            ContentSearchBackend::Ripgrep => {
                if let Ok(results) = self.search_content_with_ripgrep(query, options) {
                    return results;
                }
                // Fall back to ag if ripgrep fails
                if let Ok(results) = self.search_content_with_ag(query, options) {
                    return results;
                }
                // Final fallback to original implementation
                self.search_content_fallback(query, options)
            }
            ContentSearchBackend::Ag => {
                if let Ok(results) = self.search_content_with_ag(query, options) {
                    return results;
                }
                // Fall back to original implementation
                self.search_content_fallback(query, options)
            }
            ContentSearchBackend::Fallback => {
                self.search_content_fallback(query, options)
            }
        }
    }

    // Original implementation as fallback
    fn search_content_fallback(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let mut processed_files = std::collections::HashSet::new();

        // Get unique file paths from symbols
        for symbol in &self.symbols {
            let file_path = &symbol.file;
            
            // Skip if we've already processed this file
            if !processed_files.insert(file_path.clone()) {
                continue;
            }

            // Skip binary and large files
            if !self.should_search_file_content(file_path) {
                continue;
            }

            // Read and search file content
            if let Ok(content) = fs::read_to_string(file_path) {
                let mut line_matches = Vec::new();
                
                // Search each line for literal matches (like ripgrep/ag)
                for (line_num, line) in content.lines().enumerate() {
                    // Case-insensitive literal search to match ripgrep/ag behavior
                    if line.to_lowercase().contains(&query.to_lowercase()) {
                        line_matches.push((line_num + 1, line.trim().to_string()));
                    }
                }

                // Create search results for matching lines
                for (line_num, line_content) in line_matches {
                    results.push(SearchResult {
                        symbol: CodeSymbol {
                            name: line_content.clone(),
                            symbol_type: SymbolType::Variable, // Use Variable as a generic content type
                            file: file_path.clone(),
                            line: line_num,
                            column: 1,
                            context: Some(line_content),
                        },
                        score: 0.2, // Fallback results have lower quality than ripgrep/ag (0.05/0.15)
                    });
                }
            }
        }

        // Sort by score (ascending = better match first)
        results.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        results
    }

    // Check if file should be searched for content
    fn should_search_file_content(&self, file_path: &Path) -> bool {
        // Check file size - skip files larger than 1MB
        const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
        if let Ok(metadata) = file_path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                return false;
            }
        }

        // Skip binary files
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            let binary_extensions = [
                "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp",
                "zip", "tar", "gz", "bz2", "7z", "rar",
                "exe", "bin", "so", "dylib", "dll", "app",
                "mp3", "mp4", "avi", "mov", "wmv", "flv",
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
                "db", "sqlite", "sqlite3",
                "ttf", "otf", "woff", "woff2",
                "o", "obj", "pyc", "class", "jar",
                "lock"
            ];
            
            if binary_extensions.contains(&extension.to_lowercase().as_str()) {
                return false;
            }
        }

        true
    }
}