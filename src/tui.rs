use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use arboard::Clipboard;

use crate::{
    searcher::FuzzySearcher,
    types::{CodeSymbol, DefaultDisplayStrategy, SearchOptions, SearchResult, SymbolType, IndexUpdate},
    file_watcher::FileWatcher,
    indexer::TreeSitterIndexer,
};

type IndexingReceiver = std::sync::mpsc::Receiver<(Vec<CodeSymbol>, u32, usize, usize, bool)>;

#[derive(Debug, Clone)]
pub struct SearchMode {
    pub name: String,
    pub prefix: String,
    pub icon: String,
}

pub struct TuiApp {
    pub searcher: Option<FuzzySearcher>,
    pub symbols: Vec<CodeSymbol>,
    pub current_results: Vec<SearchResult>,
    pub selected_index: usize,
    pub query: String,
    pub current_search_mode: SearchMode,
    pub search_modes: Vec<SearchMode>,
    pub should_quit: bool,
    pub show_help: bool,
    pub status_message: String,
    pub default_strategy: DefaultDisplayStrategy,
    pub indexing_receiver: Option<IndexingReceiver>,
    pub is_indexing: bool,
    pub indexing_start_time: Option<std::time::Instant>,
    // File watching components
    pub file_watcher: Option<FileWatcher>,
    pub indexer: Option<TreeSitterIndexer>,
    pub watch_enabled: bool,
    pub last_updated_file: Option<String>,
    // Cache management
    pub directory_path: Option<std::path::PathBuf>,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    pub fn new() -> Self {
        let search_modes = vec![
            SearchMode {
                name: "Content".to_string(),
                prefix: "".to_string(),
                icon: "🔍".to_string(),
            },
            SearchMode {
                name: "Symbol".to_string(),
                prefix: "#".to_string(),
                icon: "🏷️".to_string(),
            },
            SearchMode {
                name: "File".to_string(),
                prefix: ">".to_string(),
                icon: "📁".to_string(),
            },
            SearchMode {
                name: "Regex".to_string(),
                prefix: "/".to_string(),
                icon: "🔧".to_string(),
            },
        ];

        Self {
            searcher: None,
            symbols: Vec::new(),
            current_results: Vec::new(),
            selected_index: 0,
            query: String::new(),
            current_search_mode: search_modes[0].clone(),
            search_modes,
            should_quit: false,
            show_help: false,
            status_message: "Ready".to_string(),
            default_strategy: DefaultDisplayStrategy::RecentlyModified,
            indexing_receiver: None,
            is_indexing: false,
            indexing_start_time: None,
            // File watching components
            file_watcher: None,
            indexer: None,
            watch_enabled: false,
            last_updated_file: None,
            // Cache management
            directory_path: None,
        }
    }

    pub async fn initialize(&mut self, directory: &std::path::Path, verbose: bool, respect_gitignore: bool) -> anyhow::Result<()> {
        // Store directory path for cache management
        self.directory_path = Some(directory.to_path_buf());
        
        // Phase 1: Load cache symbols immediately for instant availability
        self.status_message = "Loading cache...".to_string();
        let cache_symbols = self.load_cache_symbols(directory, verbose).await?;
        
        // Phase 2: Quick file discovery for immediate display
        self.status_message = "Discovering files...".to_string();
        let file_list = self.quick_file_discovery(directory, respect_gitignore).await?;
        
        // Phase 3: Merge cache symbols with file symbols, avoiding duplicates
        let mut initial_symbols = self.create_file_symbols(&file_list);
        
        // Add cache symbols, but avoid duplicates by checking file paths
        let mut existing_files: std::collections::HashSet<std::path::PathBuf> = 
            initial_symbols.iter().map(|s| s.file.clone()).collect();
        
        let cache_count = cache_symbols.len();
        for cache_symbol in cache_symbols {
            if !existing_files.contains(&cache_symbol.file) {
                existing_files.insert(cache_symbol.file.clone());
                initial_symbols.push(cache_symbol);
            }
        }
        
        self.symbols = initial_symbols;
        self.searcher = Some(FuzzySearcher::new(self.symbols.clone()));
        
        // Show initial results immediately (now includes cache symbols)
        self.show_default_results();
        let cache_info = if cache_count > 0 {
            format!(" (including {} cached symbols)", cache_count)
        } else {
            String::new()
        };
        self.status_message = format!("Found {} files{}, indexing symbols...", file_list.len(), cache_info);
        
        // Phase 4: Start progressive symbol indexing in background (non-blocking)
        self.start_progressive_indexing(directory, verbose, respect_gitignore, file_list);
        
        Ok(())
    }
    
    /// Load symbols from cache for immediate availability in TUI
    async fn load_cache_symbols(&self, directory: &std::path::Path, verbose: bool) -> anyhow::Result<Vec<CodeSymbol>> {
        use crate::indexer::TreeSitterIndexer;
        
        let mut indexer = TreeSitterIndexer::with_options(verbose, true);
        indexer.initialize_sync()?;
        
        match indexer.load_cache(directory) {
            Ok(stats) if stats.total_symbols > 0 => {
                if verbose {
                    eprintln!("TUI cache loaded: {} files, {} symbols", stats.total_files, stats.total_symbols);
                }
                
                // Extract all symbols from cache for immediate use
                let cache_symbols = indexer.get_all_cached_symbols();
                
                if verbose && !cache_symbols.is_empty() {
                    eprintln!("Extracted {} symbols from cache for immediate TUI use", cache_symbols.len());
                }
                
                Ok(cache_symbols)
            }
            Ok(_) => {
                if verbose {
                    eprintln!("No cache available for TUI");
                }
                Ok(Vec::new())
            }
            Err(e) => {
                if verbose {
                    eprintln!("Failed to load cache for TUI: {}", e);
                }
                Ok(Vec::new())
            }
        }
    }

    pub async fn initialize_with_watch(&mut self, directory: &std::path::Path, verbose: bool, respect_gitignore: bool, watch_enabled: bool) -> anyhow::Result<()> {
        // Initialize normally first
        self.initialize(directory, verbose, respect_gitignore).await?;
        
        // Set up file watching if enabled
        self.watch_enabled = watch_enabled;
        if watch_enabled {
            self.setup_file_watching(directory, verbose, respect_gitignore)?;
        }
        
        Ok(())
    }

    fn setup_file_watching(&mut self, directory: &std::path::Path, verbose: bool, respect_gitignore: bool) -> anyhow::Result<()> {
        // Initialize indexer for file watching
        let mut indexer = TreeSitterIndexer::with_options(verbose, respect_gitignore);
        
        // Load cache if available
        if let Ok(stats) = indexer.load_cache(directory) {
            if stats.total_files > 0 {
                self.status_message = format!("Cache loaded: {} files, {} symbols", stats.total_files, stats.total_symbols);
            }
        }
        
        indexer.initialize_sync()?;
        self.indexer = Some(indexer);

        // Set up file watcher with patterns
        let patterns = vec!["**/*".to_string()]; // Watch all files
        let file_watcher = FileWatcher::new(directory, patterns, Some(100))?; // 100ms debounce
        self.file_watcher = Some(file_watcher);

        if verbose {
            self.status_message = "File watching enabled - index will update automatically".to_string();
        }

        Ok(())
    }
    
    // Quick file discovery without symbol extraction
    async fn quick_file_discovery(&self, directory: &std::path::Path, respect_gitignore: bool) -> anyhow::Result<Vec<std::path::PathBuf>> {
        use ignore::WalkBuilder;
        
        let mut builder = WalkBuilder::new(directory);
        builder.git_ignore(respect_gitignore)
               .git_global(respect_gitignore)
               .git_exclude(respect_gitignore)
               .require_git(false)
               .hidden(false)
               .parents(true)
               .ignore(true)
               .add_custom_ignore_filename(".ignore");
        
        let mut files_with_time = Vec::new();
        
        for entry in builder.build() {
            match entry {
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    
                    // Skip .git directory
                    if let Some(path_str) = path.to_str() {
                        if path_str.contains("/.git/") || path_str.ends_with("/.git") {
                            continue;
                        }
                    }
                    
                    if path.is_file() && self.should_include_file_quick(path) {
                        if let Ok(metadata) = path.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                files_with_time.push((path.to_path_buf(), modified));
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        
        // Sort by modification time (newest first)
        files_with_time.sort_by(|a, b| b.1.cmp(&a.1));
        
        Ok(files_with_time.into_iter().map(|(path, _)| path).collect())
    }
    
    // Quick file filtering without full indexer logic
    fn should_include_file_quick(&self, path: &std::path::Path) -> bool {
        // Basic file size check
        const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                return false;
            }
        }
        
        // Skip binary files
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
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
    
    // Create basic file symbols for immediate display
    fn create_file_symbols(&self, file_list: &[std::path::PathBuf]) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        
        for file_path in file_list.iter().take(100) { // Limit for performance
            // Add filename symbol
            if let Some(filename) = file_path.file_name() {
                symbols.push(CodeSymbol {
                    name: filename.to_string_lossy().to_string(),
                    symbol_type: SymbolType::Filename,
                    file: file_path.clone(),
                    line: 1,
                    column: 1,
                    context: None,
                });
            }
            
            // Add dirname symbol
            if let Some(parent) = file_path.parent() {
                if let Some(dirname) = parent.file_name() {
                    symbols.push(CodeSymbol {
                        name: dirname.to_string_lossy().to_string(),
                        symbol_type: SymbolType::Dirname,
                        file: file_path.clone(),
                        line: 1,
                        column: 1,
                        context: None,
                    });
                }
            }
        }
        
        symbols
    }
    
    // Progressive indexing in background while UI remains responsive
    fn start_progressive_indexing(&mut self, directory: &std::path::Path, verbose: bool, _respect_gitignore: bool, file_list: Vec<std::path::PathBuf>) {
        use crate::indexer::TreeSitterIndexer;
        use std::sync::mpsc;
        use std::thread;
        
        let (tx, rx) = mpsc::channel();
        let mut indexer = TreeSitterIndexer::new();
        
        // Load cache if available for smart indexing
        let cache_loaded = if let Ok(stats) = indexer.load_cache(directory) {
            if verbose {
                eprintln!("Cache loaded for progressive indexing: {} files, {} symbols", 
                         stats.total_files, stats.total_symbols);
            }
            true
        } else {
            if verbose {
                eprintln!("No cache available for progressive indexing");
            }
            false
        };
        
        let total_files = file_list.len();
        let directory_path = directory.to_path_buf();
        
        // Spawn background thread for smart symbol extraction
        thread::spawn(move || {
            let mut processed = 0;
            let mut cache_hits = 0;
            let mut cache_misses = 0;
            
            for file_path in file_list {
                // Smart cache-aware indexing: only process files that need updating
                match indexer.load_or_index_file(&file_path) {
                    Ok(symbols) => {
                        // Check if this was a cache hit or miss
                        if cache_loaded {
                            if indexer.is_file_cached_and_valid(&file_path) {
                                cache_hits += 1;
                            } else {
                                cache_misses += 1;
                            }
                        }
                        
                        processed += 1;
                        let progress = (processed as f32 / total_files as f32 * 100.0) as u32;
                        let is_completed = processed >= total_files;
                        
                        // Send symbols, progress, and completion flag back to main thread
                        if tx.send((symbols, progress, processed, total_files, is_completed)).is_err() {
                            break; // Main thread has dropped the receiver
                        }
                        
                        // Save cache when indexing is complete
                        if is_completed {
                            if let Err(e) = indexer.save_cache(&directory_path) {
                                if verbose {
                                    eprintln!("Warning: Failed to save cache in background thread: {}", e);
                                }
                            } else if verbose {
                                eprintln!("Cache saved successfully after progressive indexing");
                            }
                        }
                    }
                    Err(_) => {
                        // Continue with next file on error
                        processed += 1;
                        let progress = (processed as f32 / total_files as f32 * 100.0) as u32;
                        let is_completed = processed >= total_files;
                        
                        if tx.send((Vec::new(), progress, processed, total_files, is_completed)).is_err() {
                            break;
                        }
                        
                        // Save cache even if last file failed
                        if is_completed {
                            if let Err(e) = indexer.save_cache(&directory_path) {
                                if verbose {
                                    eprintln!("Warning: Failed to save cache in background thread: {}", e);
                                }
                            } else if verbose {
                                eprintln!("Cache saved successfully after progressive indexing");
                            }
                        }
                    }
                }
            }
            
            // Log cache efficiency if verbose
            if verbose && cache_loaded && (cache_hits + cache_misses) > 0 {
                let cache_hit_rate = (cache_hits as f32 / (cache_hits + cache_misses) as f32 * 100.0) as u32;
                eprintln!("Progressive indexing completed - Cache hits: {}, Cache misses: {}, Hit rate: {}%", 
                         cache_hits, cache_misses, cache_hit_rate);
            }
        });
        
        // Store receiver for polling in main loop
        self.indexing_receiver = Some(rx);
        self.is_indexing = true;
        self.indexing_start_time = Some(std::time::Instant::now());
        
        // Update status to indicate smart background indexing has started
        let indexing_type = if cache_loaded { "smart indexing" } else { "indexing" };
        self.status_message = format!("Starting {} of {} files in background...", indexing_type, total_files);
    }
    
    // Check for background indexing updates (non-blocking)
    pub fn update_indexing_progress(&mut self) {
        if !self.is_indexing {
            return;
        }
        
        // Take receiver temporarily to avoid borrowing issues
        if let Some(receiver) = self.indexing_receiver.take() {
            let mut finished = false;
            
            // Use try_recv to avoid blocking - this is key for non-spinning behavior
            // Limit the number of updates per frame to maintain UI responsiveness
            let mut updates_this_frame = 0;
            const MAX_UPDATES_PER_FRAME: usize = 3;
            
            while let Ok((new_symbols, progress, processed, total, is_completed)) = receiver.try_recv() {
                // Add new symbols to our collection
                self.symbols.extend(new_symbols);
                updates_this_frame += 1;
                
                // Only update expensive operations periodically or when finished
                let should_update_searcher = is_completed || updates_this_frame >= MAX_UPDATES_PER_FRAME;
                
                if should_update_searcher {
                    // Update searcher with new symbols
                    self.searcher = Some(crate::searcher::FuzzySearcher::new(self.symbols.clone()));
                    
                    // Update default results if query is empty
                    if self.query.trim().is_empty() {
                        self.show_default_results();
                    }
                }
                
                // Update status with progress
                if is_completed {
                    // Calculate indexing duration
                    let duration = if let Some(start_time) = self.indexing_start_time {
                        start_time.elapsed()
                    } else {
                        std::time::Duration::from_secs(0)
                    };
                    
                    let duration_ms = duration.as_millis();
                    
                    self.status_message = if duration_ms < 1000 {
                        format!("Indexing complete! Found {} symbols ({}ms, with cache)", self.symbols.len(), duration_ms)
                    } else {
                        format!("Indexing complete! Found {} symbols ({:.1}s, with cache)", self.symbols.len(), duration.as_secs_f64())
                    };
                    
                    self.is_indexing = false;
                    self.indexing_start_time = None;
                    finished = true;
                    break;
                } else if should_update_searcher {
                    // Only update status message periodically to reduce UI churn
                    self.status_message = format!("Smart indexing progress: {}/{} files ({}%)", processed, total, progress);
                }
                
                // Break early if we've hit our update limit for this frame
                if updates_this_frame >= MAX_UPDATES_PER_FRAME && processed < total {
                    break;
                }
            }
            
            // Put receiver back if not finished
            if !finished {
                self.indexing_receiver = Some(receiver);
            }
        }

        // Also check for file watcher updates
        self.process_file_watcher_events();
    }

    /// Process file watcher events and apply index updates
    fn process_file_watcher_events(&mut self) {
        if !self.watch_enabled || self.file_watcher.is_none() || self.indexer.is_none() {
            return;
        }

        let mut updates_processed = 0;
        const MAX_WATCH_UPDATES_PER_FRAME: usize = 5;

        // Process file watcher updates
        while updates_processed < MAX_WATCH_UPDATES_PER_FRAME {
            // Try to get an update, avoiding long borrows
            let index_update = if let Some(ref file_watcher) = self.file_watcher {
                match file_watcher.try_recv_update() {
                    Ok(Some(update)) => update,
                    Ok(None) => break, // No more updates
                    Err(_) => break,   // Channel disconnected or other error
                }
            } else {
                break;
            };

            // Now process the update without borrowing file_watcher
            if let Some(ref mut indexer) = self.indexer {
                if let Err(e) = indexer.apply_index_update(&index_update) {
                    eprintln!("Failed to apply index update: {}", e);
                    continue;
                }
                
                // Update our symbols cache with the new symbols from indexer
                self.symbols = indexer.get_all_symbols();
                
                // Update searcher with new symbols
                self.searcher = Some(FuzzySearcher::new(self.symbols.clone()));
                
                // Update current results if we have a query
                if !self.query.trim().is_empty() {
                    self.perform_search();
                } else {
                    self.show_default_results();
                }
                
                // Update status message for file events and track last updated file
                match &index_update {
                    IndexUpdate::Added { file, symbols } => {
                        let filename = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                        self.last_updated_file = Some(filename.clone());
                        self.status_message = format!("File added: {} ({} symbols)", filename, symbols.len());
                    }
                    IndexUpdate::Modified { file, symbols } => {
                        let filename = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                        self.last_updated_file = Some(filename.clone());
                        self.status_message = format!("File modified: {} ({} symbols)", filename, symbols.len());
                    }
                    IndexUpdate::Removed { file, symbol_count } => {
                        let filename = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                        self.last_updated_file = Some(filename.clone());
                        self.status_message = format!("File deleted: {} ({} symbols removed)", filename, symbol_count);
                    }
                }
                
                updates_processed += 1;
            }
        }
    }

    fn detect_search_mode(&self, query: &str) -> SearchMode {
        if query.starts_with('#') {
            return self.search_modes[1].clone(); // Symbol
        } else if query.starts_with('>') {
            return self.search_modes[2].clone(); // File
        } else if query.starts_with('/') {
            return self.search_modes[3].clone(); // Regex
        }
        self.search_modes[0].clone() // Content (default)
    }

    fn extract_search_query(&self, query: &str) -> String {
        match &self.current_search_mode.prefix {
            prefix if !prefix.is_empty() && query.starts_with(prefix) => {
                query[prefix.len()..].to_string()
            }
            _ => query.to_string(),
        }
    }

    fn show_default_results(&mut self) {
        if self.symbols.is_empty() {
            return;
        }

        let default_symbols = self.sort_symbols_by_strategy(&self.default_strategy);
        
        // Deduplicate by file path, prioritizing filename symbols over code symbols
        let mut file_representatives: std::collections::HashMap<std::path::PathBuf, CodeSymbol> = std::collections::HashMap::new();
        let mut seen_dirs = std::collections::HashSet::new();
        
        // First pass: collect the best representative symbol for each file
        for symbol in default_symbols {
            match symbol.symbol_type {
                SymbolType::Dirname => {
                    // Always include unique directory names
                    if seen_dirs.insert(symbol.name.clone()) {
                        file_representatives.insert(symbol.file.clone(), symbol);
                    }
                }
                SymbolType::Filename => {
                    // Filename symbols are the best representatives for files
                    file_representatives.insert(symbol.file.clone(), symbol);
                }
                _ => {
                    // For code symbols, only use if we don't have a filename symbol yet
                    file_representatives.entry(symbol.file.clone()).or_insert(symbol);
                }
            }
        }
        
        // Convert back to sorted vector based on the original sort strategy
        let mut unique_symbols: Vec<CodeSymbol> = file_representatives.into_values().collect();
        
        // Re-sort by modification time (since HashMap iteration order is not guaranteed)
        if matches!(self.default_strategy, DefaultDisplayStrategy::RecentlyModified) {
            use std::collections::HashMap;
            let mut file_times: HashMap<std::path::PathBuf, std::time::SystemTime> = HashMap::new();
            
            for symbol in &unique_symbols {
                if let Ok(metadata) = std::fs::metadata(&symbol.file) {
                    if let Ok(modified) = metadata.modified() {
                        file_times.insert(symbol.file.clone(), modified);
                    }
                }
            }
            
            unique_symbols.sort_by(|a, b| {
                let time_a = file_times.get(&a.file).copied().unwrap_or(std::time::UNIX_EPOCH);
                let time_b = file_times.get(&b.file).copied().unwrap_or(std::time::UNIX_EPOCH);
                time_b.cmp(&time_a) // Most recent first
            });
        }
        
        self.current_results = unique_symbols
            .into_iter()
            .take(100) // Limit to 100 results
            .map(|s| SearchResult {
                symbol: s,
                score: 1.0,
            })
            .collect();
        
        self.selected_index = 0;
    }

    fn sort_symbols_by_strategy(&self, strategy: &DefaultDisplayStrategy) -> Vec<CodeSymbol> {
        match strategy {
            DefaultDisplayStrategy::RecentlyModified => {
                self.sort_by_recent_modification()
            }
            DefaultDisplayStrategy::ProjectImportant => {
                self.sort_by_project_importance()
            }
            DefaultDisplayStrategy::SymbolBalance => {
                self.sort_by_symbol_balance()
            }
            DefaultDisplayStrategy::MostSymbols => {
                self.sort_by_most_symbols()
            }
            DefaultDisplayStrategy::Random => {
                self.sort_by_random()
            }
        }
    }

    fn sort_by_recent_modification(&self) -> Vec<CodeSymbol> {
        use std::collections::HashMap;
        
        // Group symbols by file and get file modification times
        let mut file_times: HashMap<std::path::PathBuf, std::time::SystemTime> = HashMap::new();
        
        for symbol in &self.symbols {
            if !file_times.contains_key(&symbol.file) {
                if let Ok(metadata) = std::fs::metadata(&symbol.file) {
                    if let Ok(modified) = metadata.modified() {
                        file_times.insert(symbol.file.clone(), modified);
                    }
                }
            }
        }
        
        // Sort symbols by file modification time (most recent first)
        let mut sorted_symbols = self.symbols.clone();
        sorted_symbols.sort_by(|a, b| {
            let time_a = file_times.get(&a.file).copied().unwrap_or(std::time::UNIX_EPOCH);
            let time_b = file_times.get(&b.file).copied().unwrap_or(std::time::UNIX_EPOCH);
            time_b.cmp(&time_a) // Reverse order for most recent first
        });
        
        sorted_symbols
    }

    fn sort_by_project_importance(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_symbol_balance(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_most_symbols(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_random(&self) -> Vec<CodeSymbol> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut symbols = self.symbols.clone();
        symbols.shuffle(&mut rng);
        symbols
    }

    pub fn perform_search(&mut self) {
        if let Some(ref searcher) = self.searcher {
            // Detect and update search mode
            self.current_search_mode = self.detect_search_mode(&self.query);
            
            if self.query.trim().is_empty() {
                // Use the same deduplication logic as show_default_results
                self.show_default_results();
            } else {
                let mut clean_query = self.extract_search_query(&self.query);
                
                // Store whether the query was a directory search (ended with '/')
                let is_directory_search = clean_query.ends_with('/');
                
                // Remove trailing '/' for the actual search if it exists
                if is_directory_search {
                    clean_query = clean_query.trim_end_matches('/').to_string();
                }
                
                self.current_results = match self.current_search_mode.name.as_str() {
                    "Symbol" => {
                        let search_options = SearchOptions {
                            include_files: Some(false),
                            include_dirs: Some(false),
                            types: Some(vec![
                                SymbolType::Function,
                                SymbolType::Variable,
                                SymbolType::Class,
                                SymbolType::Interface,
                                SymbolType::Type,
                                SymbolType::Enum,
                                SymbolType::Constant,
                                SymbolType::Method,
                                SymbolType::Property,
                            ]),
                            ..Default::default()
                        };
                        searcher.search(&clean_query, &search_options)
                    },
                    "File" => {
                        // Check if the query ended with '/' for directory-only search
                        let search_options = if is_directory_search {
                            SearchOptions {
                                types: Some(vec![SymbolType::Dirname]),
                                ..Default::default()
                            }
                        } else {
                            SearchOptions {
                                types: Some(vec![SymbolType::Filename, SymbolType::Dirname]),
                                ..Default::default()
                            }
                        };
                        searcher.search(&clean_query, &search_options)
                    },
                    "Regex" => {
                        // Regex search on file contents
                        let search_options = SearchOptions::default();
                        searcher.search_content(&clean_query, &search_options)
                    },
                    _ => {
                        // Default "Content" mode: search file contents
                        let search_options = SearchOptions::default();
                        searcher.search_content(&clean_query, &search_options)
                    }
                };
            }
            
            self.selected_index = 0;
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+P: Move up (previous)
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+N: Move down (next)
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                self.copy_current_result();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.perform_search();
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.perform_search();
            }
            _ => {}
        }
    }

    fn copy_current_result(&mut self) {
        if let Some(result) = self.current_results.get(self.selected_index) {
            let location = format!(
                "{}:{}:{}",
                result.symbol.file.display(),
                result.symbol.line,
                result.symbol.column
            );
            
            // Temporarily disable raw mode and restore normal terminal state for clipboard operation
            let clipboard_result = {
                // Disable raw mode temporarily
                if let Err(e) = disable_raw_mode() {
                    Some(format!("❌ Failed to disable raw mode: {}", e))
                } else {
                    // Perform clipboard operation in normal mode
                    let result = match Clipboard::new() {
                        Ok(mut clipboard) => {
                            match clipboard.set_text(&location) {
                                Ok(_) => None, // Success
                                Err(e) => Some(format!("❌ Failed to copy: {}", e)),
                            }
                        }
                        Err(e) => Some(format!("❌ Failed to access clipboard: {}", e)),
                    };
                    
                    // Re-enable raw mode
                    if let Err(e) = enable_raw_mode() {
                        Some(format!("❌ Failed to re-enable raw mode: {}", e))
                    } else {
                        result
                    }
                }
            };
            
            // Set status message based on result
            match clipboard_result {
                Some(error_msg) => {
                    self.status_message = error_msg;
                }
                None => {
                    self.status_message = format!("📋 Copied: {}", location);
                }
            }
            
            // Clear search box
            self.query.clear();
            self.perform_search();
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search box
                Constraint::Min(1),    // Results
                Constraint::Length(3), // Status/help
            ])
            .split(f.size());

        self.render_search_box(f, chunks[0]);
        self.render_results(f, chunks[1]);
        self.render_status(f, chunks[2]);

        if self.show_help {
            self.render_help_popup(f);
        }
    }

    fn render_search_box(&self, f: &mut Frame, area: Rect) {
        let mode_info = if self.query.is_empty() && self.current_search_mode.name == "Content" {
            format!("{} Recently Edited", self.current_search_mode.icon)
        } else {
            format!("{} {} Search", self.current_search_mode.icon, self.current_search_mode.name)
        };
        
        let search_text = Text::from(vec![
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Cyan)),
                Span::raw(&self.query),
                Span::styled("_", Style::default().fg(Color::Yellow)), // Cursor
            ]),
        ]);

        let search_box = Paragraph::new(search_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(mode_info)
                    .border_style(Style::default().fg(Color::Blue)),
            );

        f.render_widget(search_box, area);
    }

    fn render_results(&mut self, f: &mut Frame, area: Rect) {
        // Show special message during indexing only if no results and user has typed something
        if self.is_indexing && !self.query.trim().is_empty() && self.current_results.is_empty() {
            let indexing_message = ListItem::new(Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled("Indexing in progress... Results will appear as files are processed", 
                           Style::default().fg(Color::Gray)),
            ]));
            
            let list = List::new(vec![indexing_message])
                .block(Block::default().borders(Borders::ALL).title("Results"))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            
            f.render_widget(list, area);
            return;
        }
        
        let items: Vec<ListItem> = self
            .current_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let icon = match result.symbol.symbol_type {
                    SymbolType::Function => "🔧",
                    SymbolType::Variable => "📦",
                    SymbolType::Class => "🏗️",
                    SymbolType::Interface => "📐",
                    SymbolType::Type => "🔖",
                    SymbolType::Enum => "📝",
                    SymbolType::Constant => "🔒",
                    SymbolType::Method => "⚙️",
                    SymbolType::Property => "🔹",
                    SymbolType::Filename => "📄",
                    SymbolType::Dirname => "📁",
                };

                let line = Line::from(vec![
                    Span::raw(format!("{} ", icon)),
                    Span::styled(
                        &result.symbol.name,
                        if i == self.selected_index {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!(" ({}:{})", result.symbol.file.display(), result.symbol.line),
                        Style::default().fg(Color::Gray),
                    ),
                ]);

                ListItem::new(line).style(
                    if i == self.selected_index {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    }
                )
            })
            .collect();

        let results_count = if self.is_indexing {
            format!("Results: {} (indexing...)", self.current_results.len())
        } else {
            format!("Results: {}", self.current_results.len())
        };
        let results_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(results_count)
                    .border_style(Style::default().fg(Color::Green)),
            );

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_index));

        f.render_stateful_widget(results_list, area, &mut list_state);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let mut status_spans = vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
            Span::raw(&self.status_message),
            if self.watch_enabled {
                Span::styled(" | 👁 Watch: ON", Style::default().fg(Color::Green))
            } else {
                Span::styled(" | 👁 Watch: OFF", Style::default().fg(Color::Red))
            },
        ];

        // Add last updated file information if available
        if let Some(ref last_file) = self.last_updated_file {
            status_spans.push(Span::styled(" | Last: ", Style::default().fg(Color::Blue)));
            status_spans.push(Span::styled(last_file, Style::default().fg(Color::White)));
        }

        let status_text = Text::from(vec![
            Line::from(status_spans),
            Line::from(vec![
                Span::styled("Keys: ", Style::default().fg(Color::Yellow)),
                Span::raw("↑/↓/C-p/C-n Navigate • Enter Copy • ? Help • Esc/C-c Quit"),
            ]),
        ]);

        let status_box = Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Status")
                    .border_style(Style::default().fg(Color::Magenta)),
            );

        f.render_widget(status_box, area);
    }

    fn render_help_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(60, 70, f.size());

        let help_text = Text::from(vec![
            Line::from("Symbol Fuzzy Search - Help"),
            Line::from(""),
            Line::from("Search Modes:"),
            Line::from("  🔍 Content - Search file contents (default)"),
            Line::from("  🏷️ #symbol - Search code symbols only"),
            Line::from("  📁 >file - Search filenames/directories"),
            Line::from("  📁 >dir/ - Search directories only (with trailing /)"),
            Line::from("  🔧 /regex - Regular expression on file contents"),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/↓ or Ctrl+P/Ctrl+N - Move selection"),
            Line::from("  Enter - Copy location to clipboard"),
            Line::from("  Backspace - Delete character"),
            Line::from("  ? - Toggle this help"),
            Line::from("  Esc / Ctrl+C - Quit"),
            Line::from(""),
            Line::from("File Watching:"),
            Line::from(format!("  Status: {}", if self.watch_enabled { "🟢 Enabled" } else { "🔴 Disabled" })),
            Line::from("  Monitors files for real-time index updates"),
            Line::from(""),
            Line::from("Note: By default, files ignored by .gitignore are excluded."),
            Line::from("      Use --include-ignored flag to search all files."),
            Line::from(""),
            Line::from("Press any key to close help"),
        ]);

        let help_popup = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().bg(Color::Black));

        f.render_widget(Clear, popup_area);
        f.render_widget(help_popup, popup_area);
    }
}

// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub async fn run_tui(directory: std::path::PathBuf, verbose: bool, respect_gitignore: bool) -> anyhow::Result<()> {
    run_tui_with_watch(directory, verbose, respect_gitignore, false).await
}

pub async fn run_tui_with_watch(directory: std::path::PathBuf, verbose: bool, respect_gitignore: bool, watch_enabled: bool) -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and initialize app
    let mut app = TuiApp::new();
    app.initialize_with_watch(&directory, verbose, respect_gitignore, watch_enabled).await?;

    // Main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
    loop {
        // Update indexing progress from background thread (non-blocking)
        app.update_indexing_progress();
        
        terminal.draw(|f| app.render(f))?;

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }
    }

    Ok(())
}
