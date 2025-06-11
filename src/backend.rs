use crate::{
    indexer::TreeSitterIndexer,
    searcher::FuzzySearcher,
    file_watcher::FileWatcher,
    types::{CodeSymbol, SearchResult, SearchMode, SearchOptions},
};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use anyhow::Result;

/// Events sent from backend to UI
#[derive(Debug, Clone)]
pub enum BackendEvent {
    IndexingProgress { 
        processed: usize, 
        total: usize, 
        symbols: Vec<CodeSymbol> 
    },
    IndexingComplete { 
        duration: Duration, 
        total_symbols: usize 
    },
    FileChanged { 
        file: PathBuf, 
        change_type: FileChangeType 
    },
    SearchResults { 
        query: String, 
        results: Vec<SearchResult> 
    },
    Error { 
        message: String 
    },
}

/// Commands sent from UI to backend
#[derive(Debug, Clone)]
pub enum UserCommand {
    StartIndexing { 
        directory: PathBuf 
    },
    Search { 
        query: String, 
        mode: SearchMode 
    },
    EnableFileWatching,
    CopyResult { 
        index: usize 
    },
    Quit,
}

/// File change types for file watching
#[derive(Debug, Clone)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

/// Independent search backend with no UI dependencies
pub struct SearchBackend {
    indexer: TreeSitterIndexer,
    searcher: Option<FuzzySearcher>,
    file_watcher: Option<FileWatcher>,
    event_sender: mpsc::Sender<BackendEvent>,
    command_receiver: mpsc::Receiver<UserCommand>,
    is_indexing: bool,
    indexing_start_time: Option<Instant>,
    directory_path: Option<PathBuf>,
    verbose: bool,
    respect_gitignore: bool,
}

impl SearchBackend {
    pub fn new(
        verbose: bool,
        respect_gitignore: bool,
    ) -> (Self, mpsc::Sender<UserCommand>, mpsc::Receiver<BackendEvent>) {
        let (event_sender, event_receiver) = mpsc::channel();
        let (command_sender, command_receiver) = mpsc::channel();
        
        let backend = Self {
            indexer: TreeSitterIndexer::with_options(verbose, respect_gitignore),
            searcher: None,
            file_watcher: None,
            event_sender,
            command_receiver,
            is_indexing: false,
            indexing_start_time: None,
            directory_path: None,
            verbose,
            respect_gitignore,
        };
        
        (backend, command_sender, event_receiver)
    }
    
    /// Main event loop for backend processing
    pub fn run(&mut self) -> Result<()> {
        // Initialize indexer
        self.indexer.initialize_sync()?;
        
        loop {
            // Process commands from UI
            match self.command_receiver.try_recv() {
                Ok(command) => {
                    match self.handle_command(command) {
                        Ok(should_quit) => {
                            if should_quit {
                                break; // Exit loop on Quit command
                            }
                        }
                        Err(e) => {
                            let _ = self.event_sender.send(BackendEvent::Error {
                                message: format!("Command handling error: {}", e),
                            });
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No commands pending, continue processing
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // UI has disconnected, exit
                    break;
                }
            }
            
            // Process file watcher events if enabled
            if let Err(e) = self.process_file_watcher_events() {
                let _ = self.event_sender.send(BackendEvent::Error {
                    message: format!("File watching error: {}", e),
                });
            }
            
            // Process indexing progress if running
            if let Err(e) = self.process_indexing_progress() {
                let _ = self.event_sender.send(BackendEvent::Error {
                    message: format!("Indexing progress error: {}", e),
                });
            }
            
            // Small delay to prevent busy waiting
            std::thread::sleep(Duration::from_millis(16));
        }
        
        Ok(())
    }
    
    fn handle_command(&mut self, command: UserCommand) -> Result<bool> {
        match command {
            UserCommand::StartIndexing { directory } => {
                self.start_indexing(directory)?;
                Ok(false) // Continue running
            }
            UserCommand::Search { query, mode } => {
                self.perform_search(query, mode)?;
                Ok(false) // Continue running
            }
            UserCommand::EnableFileWatching => {
                self.enable_file_watching()?;
                Ok(false) // Continue running
            }
            UserCommand::CopyResult { index: _ } => {
                // Backend doesn't handle clipboard operations
                // This would be handled by the UI layer
                Ok(false) // Continue running
            }
            UserCommand::Quit => {
                Ok(true) // Signal to exit
            }
        }
    }
    
    fn start_indexing(&mut self, directory: PathBuf) -> Result<()> {
        self.directory_path = Some(directory.clone());
        self.is_indexing = true;
        self.indexing_start_time = Some(Instant::now());
        
        // Use walkdir to get files directly for simplicity
        use ignore::WalkBuilder;
        
        let mut builder = WalkBuilder::new(&directory);
        builder.git_ignore(self.respect_gitignore)
               .git_global(self.respect_gitignore)
               .git_exclude(self.respect_gitignore)
               .require_git(false)
               .hidden(false)
               .parents(true)
               .ignore(true);
        
        let mut file_paths = Vec::new();
        for entry in builder.build() {
            if let Ok(dir_entry) = entry {
                let path = dir_entry.path();
                if path.is_file() {
                    file_paths.push(path.to_path_buf());
                }
            }
        }
        
        let mut total_symbols = Vec::new();
        for file_path in &file_paths {
            if let Ok(symbols) = self.indexer.create_file_symbols(file_path) {
                total_symbols.extend(symbols);
            }
        }
        
        // Store symbols in indexer's cache
        for file_path in &file_paths {
            if let Ok(symbols) = self.indexer.create_file_symbols(file_path) {
                let _ = self.indexer.add_file_symbols(file_path, symbols);
            }
        }
        
        let duration = self.indexing_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::from_secs(0));
        
        // Update searcher with new symbols
        self.searcher = Some(FuzzySearcher::new(total_symbols.clone()));
        
        let _ = self.event_sender.send(BackendEvent::IndexingComplete {
            duration,
            total_symbols: total_symbols.len(),
        });
        
        self.is_indexing = false;
        self.indexing_start_time = None;
        
        Ok(())
    }
    
    fn perform_search(&mut self, query: String, mode: SearchMode) -> Result<()> {
        if let Some(ref searcher) = self.searcher {
            let results = match mode.name.as_str() {
                "Symbol" => {
                    let search_options = SearchOptions {
                        include_files: Some(false),
                        include_dirs: Some(false),
                        types: Some(vec![
                            crate::types::SymbolType::Function,
                            crate::types::SymbolType::Variable,
                            crate::types::SymbolType::Class,
                            crate::types::SymbolType::Interface,
                            crate::types::SymbolType::Type,
                            crate::types::SymbolType::Enum,
                            crate::types::SymbolType::Constant,
                            crate::types::SymbolType::Method,
                            crate::types::SymbolType::Property,
                        ]),
                        ..Default::default()
                    };
                    searcher.search(&query, &search_options)
                }
                "File" => {
                    let search_options = SearchOptions {
                        types: Some(vec![
                            crate::types::SymbolType::Filename,
                            crate::types::SymbolType::Dirname,
                        ]),
                        ..Default::default()
                    };
                    searcher.search(&query, &search_options)
                }
                "Regex" => {
                    let search_options = SearchOptions::default();
                    searcher.search_content(&query, &search_options)
                }
                _ => {
                    // Default "Content" mode
                    let search_options = SearchOptions::default();
                    searcher.search_content(&query, &search_options)
                }
            };
            
            let _ = self.event_sender.send(BackendEvent::SearchResults {
                query,
                results,
            });
        }
        
        Ok(())
    }
    
    fn enable_file_watching(&mut self) -> Result<()> {
        if let Some(ref directory) = self.directory_path {
            // Create basic patterns for file watching
            let patterns = vec![
                "**/*.ts".to_string(),
                "**/*.tsx".to_string(),
                "**/*.js".to_string(),
                "**/*.jsx".to_string(),
                "**/*.py".to_string(),
                "**/*.rs".to_string(),
                "**/*.go".to_string(),
                "**/*.java".to_string(),
                "**/*.c".to_string(),
                "**/*.cpp".to_string(),
                "**/*.php".to_string(),
                "**/*.rb".to_string(),
                "**/*.cs".to_string(),
                "**/*.scala".to_string(),
            ];
            
            match FileWatcher::new(directory, patterns, Some(100)) {
                Ok(watcher) => {
                    self.file_watcher = Some(watcher);
                    if self.verbose {
                        eprintln!("File watching enabled for directory: {}", directory.display());
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("Failed to enable file watching: {}", e);
                    }
                    return Err(e);
                }
            }
        } else {
            if self.verbose {
                eprintln!("Cannot enable file watching: no directory set");
            }
        }
        Ok(())
    }
    
    fn process_file_watcher_events(&mut self) -> Result<()> {
        if let Some(ref file_watcher) = self.file_watcher {
            // Try to receive file change events
            match file_watcher.try_recv_update() {
                Ok(Some(index_update)) => {
                    // Convert IndexUpdate to BackendEvent
                    match index_update {
                        crate::types::IndexUpdate::Added { file, symbols } => {
                            let _ = self.event_sender.send(BackendEvent::FileChanged {
                                file: file.clone(),
                                change_type: FileChangeType::Created,
                            });
                            // Add symbols to searcher if available
                            if let Some(ref mut searcher) = self.searcher {
                                searcher.update_symbols(symbols);
                            }
                        }
                        crate::types::IndexUpdate::Modified { file, symbols } => {
                            let _ = self.event_sender.send(BackendEvent::FileChanged {
                                file: file.clone(),
                                change_type: FileChangeType::Modified,
                            });
                            // Update symbols in searcher if available
                            if let Some(ref mut searcher) = self.searcher {
                                searcher.update_symbols(symbols);
                            }
                        }
                        crate::types::IndexUpdate::Removed { file, symbol_count: _ } => {
                            let _ = self.event_sender.send(BackendEvent::FileChanged {
                                file,
                                change_type: FileChangeType::Deleted,
                            });
                            // Remove symbols from searcher if available
                            // Note: This would require extending FuzzySearcher with remove_symbols method
                        }
                    }
                }
                Ok(None) => {
                    // No events available, continue
                }
                Err(_) => {
                    // Error or disconnected, continue
                }
            }
        }
        Ok(())
    }
    
    fn process_indexing_progress(&mut self) -> Result<()> {
        // This would be implemented for progressive indexing
        // For now, we do synchronous indexing
        Ok(())
    }
}