use crate::types::*;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::fs;
use std::path::Path;

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

    // Search file contents directly (real-time search)
    pub fn search_content(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // Handle empty query
        if query.trim().is_empty() {
            return vec![];
        }

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
                
                // Search each line for matches
                for (line_num, line) in content.lines().enumerate() {
                    if let Some((score, _)) = self.matcher.fuzzy_indices(line, query) {
                        // Convert skim score to distance
                        let mut distance = 1.0 - (score as f64 / 100.0);
                        
                        // Boost exact substring matches
                        if line.to_lowercase().contains(&query.to_lowercase()) {
                            distance *= 0.5;
                        }
                        
                        // Apply threshold filtering
                        if let Some(threshold) = options.threshold {
                            if distance > threshold {
                                continue;
                            }
                        }

                        line_matches.push((line_num + 1, line.trim().to_string(), distance));
                    }
                }

                // Create search results for matching lines
                for (line_num, line_content, distance) in line_matches {
                    results.push(SearchResult {
                        symbol: CodeSymbol {
                            name: line_content.clone(),
                            symbol_type: SymbolType::Variable, // Use Variable as a generic content type
                            file: file_path.clone(),
                            line: line_num,
                            column: 1,
                            context: Some(line_content),
                        },
                        score: distance,
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