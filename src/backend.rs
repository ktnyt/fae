use crate::{
    file_watcher::FileWatcher,
    indexer::TreeSitterIndexer,
    mode::SearchModeManager,
    searcher::SearchManager,
    types::{CodeSymbol, SearchMode, SearchOptions, SearchResult},
};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Events sent from backend to UI
#[derive(Debug, Clone)]
pub enum BackendEvent {
    IndexingProgress {
        processed: usize,
        total: usize,
        symbols: Vec<CodeSymbol>,
    },
    IndexingComplete {
        duration: Duration,
        total_symbols: usize,
    },
    FileChanged {
        file: PathBuf,
        change_type: FileChangeType,
    },
    SearchResults {
        query: String,
        results: Vec<SearchResult>,
    },
    Error {
        message: String,
    },
}

/// Commands sent from UI to backend
#[derive(Debug, Clone)]
pub enum UserCommand {
    StartIndexing { directory: PathBuf },
    Search { query: String, mode: SearchMode },
    EnableFileWatching,
    CopyResult { index: usize },
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
    searcher: Option<SearchManager>,
    mode_manager: SearchModeManager,
    file_watcher: Option<FileWatcher>,
    event_sender: mpsc::Sender<BackendEvent>,
    command_receiver: mpsc::Receiver<UserCommand>,
    is_indexing: bool,
    indexing_start_time: Option<Instant>,
    directory_path: Option<PathBuf>,
    verbose: bool,
    respect_gitignore: bool,
    // Progressive indexing
    indexing_receiver: Option<mpsc::Receiver<(Vec<CodeSymbol>, usize, usize)>>,
    indexing_thread: Option<thread::JoinHandle<()>>,
}

impl SearchBackend {
    pub fn new(
        verbose: bool,
        respect_gitignore: bool,
    ) -> (
        Self,
        mpsc::Sender<UserCommand>,
        mpsc::Receiver<BackendEvent>,
    ) {
        let (event_sender, event_receiver) = mpsc::channel();
        let (command_sender, command_receiver) = mpsc::channel();

        let backend = Self {
            indexer: TreeSitterIndexer::with_options(verbose, respect_gitignore),
            searcher: None,
            mode_manager: SearchModeManager::new(),
            file_watcher: None,
            event_sender,
            command_receiver,
            is_indexing: false,
            indexing_start_time: None,
            directory_path: None,
            verbose,
            respect_gitignore,
            indexing_receiver: None,
            indexing_thread: None,
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

        // Start progressive indexing in background thread
        let (progress_sender, progress_receiver) = mpsc::channel();
        self.indexing_receiver = Some(progress_receiver);

        let event_sender = self.event_sender.clone();
        let verbose = self.verbose;
        let respect_gitignore = self.respect_gitignore;

        let handle = thread::spawn(move || {
            if let Err(e) = Self::progressive_indexing_worker(
                directory,
                progress_sender,
                event_sender,
                verbose,
                respect_gitignore,
            ) {
                eprintln!("Progressive indexing error: {}", e);
            }
        });

        self.indexing_thread = Some(handle);

        Ok(())
    }

    fn progressive_indexing_worker(
        directory: PathBuf,
        progress_sender: mpsc::Sender<(Vec<CodeSymbol>, usize, usize)>,
        event_sender: mpsc::Sender<BackendEvent>,
        verbose: bool,
        respect_gitignore: bool,
    ) -> Result<()> {
        use ignore::WalkBuilder;

        let mut builder = WalkBuilder::new(&directory);
        builder
            .git_ignore(respect_gitignore)
            .git_global(respect_gitignore)
            .git_exclude(respect_gitignore)
            .require_git(false)
            .hidden(false)
            .parents(true)
            .ignore(true);

        // Collect all files first
        let mut file_paths = Vec::new();
        for dir_entry in builder.build().flatten() {
            let path = dir_entry.path();
            if path.is_file() {
                file_paths.push(path.to_path_buf());
            }
        }

        let total_files = file_paths.len();
        let mut all_symbols = Vec::new();
        let mut processed = 0;

        // Create a temporary indexer for this thread
        let mut indexer = TreeSitterIndexer::with_options(verbose, respect_gitignore);
        if let Err(e) = indexer.initialize_sync() {
            let _ = event_sender.send(BackendEvent::Error {
                message: format!("Failed to initialize indexer: {}", e),
            });
            return Err(e);
        }

        let start_time = Instant::now();

        // Process files progressively
        for file_path in file_paths {
            match indexer.create_file_symbols(&file_path) {
                Ok(symbols) => {
                    all_symbols.extend(symbols.clone());
                    processed += 1;

                    // Send progress update
                    let _ = progress_sender.send((symbols, processed, total_files));

                    // Send progress event
                    let _ = event_sender.send(BackendEvent::IndexingProgress {
                        processed,
                        total: total_files,
                        symbols: all_symbols.clone(),
                    });
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Failed to process file {}: {}", file_path.display(), e);
                    }
                    processed += 1;
                    // Send progress even on error
                    let _ = progress_sender.send((Vec::new(), processed, total_files));
                }
            }

            // Small delay to allow other processing
            thread::sleep(Duration::from_millis(10));
        }

        let duration = start_time.elapsed();

        // Send completion event
        let _ = event_sender.send(BackendEvent::IndexingComplete {
            duration,
            total_symbols: all_symbols.len(),
        });

        Ok(())
    }

    fn perform_search(&mut self, query: String, _mode: SearchMode) -> Result<()> {
        if let Some(ref searcher) = self.searcher {
            let search_options = SearchOptions::default();
            
            // Use the new mode manager to handle search
            let (results, _mode_metadata) = self.mode_manager.search(&query, searcher, &search_options);

            let _ = self
                .event_sender
                .send(BackendEvent::SearchResults { query, results });
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
                        eprintln!(
                            "File watching enabled for directory: {}",
                            directory.display()
                        );
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("Failed to enable file watching: {}", e);
                    }
                    return Err(e);
                }
            }
        } else if self.verbose {
            eprintln!("Cannot enable file watching: no directory set");
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
                        crate::types::IndexUpdate::Removed {
                            file,
                            symbol_count: _,
                        } => {
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
        if let Some(ref receiver) = self.indexing_receiver {
            // Process up to 5 progress updates per iteration to avoid blocking
            let mut updates_processed = 0;

            while updates_processed < 5 {
                match receiver.try_recv() {
                    Ok((symbols, processed, total)) => {
                        // Update searcher with new symbols
                        if let Some(ref mut searcher) = self.searcher {
                            searcher.update_symbols(symbols);
                        } else if !symbols.is_empty() {
                            // Create initial searcher
                            self.searcher = Some(SearchManager::new(symbols));
                        }

                        updates_processed += 1;

                        // Check if indexing is complete
                        if processed >= total {
                            self.is_indexing = false;
                            self.indexing_receiver = None;
                            // Wait for thread to complete
                            if let Some(handle) = self.indexing_thread.take() {
                                let _ = handle.join();
                            }
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // No more progress updates available
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Progress thread has finished
                        self.is_indexing = false;
                        self.indexing_receiver = None;
                        if let Some(handle) = self.indexing_thread.take() {
                            let _ = handle.join();
                        }
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}
