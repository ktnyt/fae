use crate::types::{IndexUpdate, WatchEvent, WatchEventKind};
use crate::indexer::TreeSitterIndexer;
use anyhow::{Context, Result};
use notify::{EventKind, RecursiveMode, Result as NotifyResult, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Debounces file system events to prevent excessive updates during rapid changes
#[derive(Debug)]
struct EventDebouncer {
    pending_events: HashMap<PathBuf, (WatchEvent, Instant)>,
    debounce_duration: Duration,
}

impl EventDebouncer {
    fn new(debounce_ms: u64) -> Self {
        Self {
            pending_events: HashMap::new(),
            debounce_duration: Duration::from_millis(debounce_ms),
        }
    }

    /// Add an event to the debouncer, returns events that should be processed now
    fn add_event(&mut self, event: WatchEvent) -> Vec<WatchEvent> {
        let now = Instant::now();
        let mut ready_events = Vec::new();

        // Extract the path from the event
        let path = match &event {
            WatchEvent::FileChanged { path, .. } => path.clone(),
            WatchEvent::BatchUpdate { .. } => {
                // For batch updates, process immediately
                ready_events.push(event);
                return ready_events;
            }
        };

        // Check for ready events (older than debounce duration)
        let ready_paths: Vec<_> = self
            .pending_events
            .iter()
            .filter_map(|(p, (_, timestamp))| {
                if now.duration_since(*timestamp) >= self.debounce_duration {
                    Some(p.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove ready events and add them to the result
        for ready_path in ready_paths {
            if let Some((ready_event, _)) = self.pending_events.remove(&ready_path) {
                ready_events.push(ready_event);
            }
        }

        // Add or update the current event
        self.pending_events.insert(path, (event, now));

        ready_events
    }

    /// Get all remaining events (useful for shutdown)
    fn flush(&mut self) -> Vec<WatchEvent> {
        let events: Vec<_> = self
            .pending_events
            .drain()
            .map(|(_, (event, _))| event)
            .collect();
        events
    }
}

/// Manages file system watching and event processing
pub struct FileWatcher {
    _watcher: Box<dyn Watcher>,
    event_receiver: Receiver<IndexUpdate>,
    debouncer: Arc<Mutex<EventDebouncer>>,
}

impl FileWatcher {
    /// Create a new file watcher for the given directory
    pub fn new(
        watch_path: &Path,
        patterns: Vec<String>,
        debounce_ms: Option<u64>,
    ) -> Result<Self> {
        let (notify_tx, notify_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();

        // Create file system watcher
        let mut watcher = notify::recommended_watcher(notify_tx)
            .context("Failed to create file system watcher")?;

        watcher
            .watch(watch_path, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch directory: {}", watch_path.display()))?;

        let debouncer = Arc::new(Mutex::new(EventDebouncer::new(debounce_ms.unwrap_or(100))));

        // Spawn background thread to process file system events
        let debouncer_clone = Arc::clone(&debouncer);
        let patterns_clone = patterns.clone();
        let watch_path_clone = watch_path.to_path_buf();

        std::thread::spawn(move || {
            Self::process_events(
                notify_rx,
                update_tx,
                debouncer_clone,
                patterns_clone,
                watch_path_clone,
            );
        });

        Ok(FileWatcher {
            _watcher: Box::new(watcher),
            event_receiver: update_rx,
            debouncer,
        })
    }

    /// Get the receiver for index updates
    pub fn updates(&self) -> &Receiver<IndexUpdate> {
        &self.event_receiver
    }

    /// Try to receive an index update without blocking
    pub fn try_recv_update(&self) -> Result<Option<IndexUpdate>, mpsc::TryRecvError> {
        match self.event_receiver.try_recv() {
            Ok(update) => Ok(Some(update)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Flush any pending debounced events
    pub fn flush_pending(&self) -> Vec<WatchEvent> {
        if let Ok(mut debouncer) = self.debouncer.lock() {
            debouncer.flush()
        } else {
            Vec::new()
        }
    }

    /// Background thread function to process notify events
    fn process_events(
        notify_rx: Receiver<NotifyResult<notify::Event>>,
        update_tx: Sender<IndexUpdate>,
        debouncer: Arc<Mutex<EventDebouncer>>,
        patterns: Vec<String>,
        watch_path: PathBuf,
    ) {
        while let Ok(event_result) = notify_rx.recv() {
            match event_result {
                Ok(event) => {
                    if let Some(watch_event) = Self::convert_notify_event(event, &watch_path) {
                        // Check if this file matches our patterns
                        if let WatchEvent::FileChanged { path, .. } = &watch_event {
                            if !Self::should_index_file(path, &patterns) {
                                continue;
                            }
                        }

                        // Add to debouncer and process ready events
                        if let Ok(mut debouncer_guard) = debouncer.lock() {
                            let ready_events = debouncer_guard.add_event(watch_event);
                            for ready_event in ready_events {
                                if let Some(index_update) =
                                    Self::convert_to_index_update(ready_event, &patterns)
                                {
                                    if update_tx.send(index_update).is_err() {
                                        // Receiver has been dropped, exit thread
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("File watcher error: {}", err);
                }
            }
        }
    }

    /// Convert notify event to our WatchEvent
    fn convert_notify_event(event: notify::Event, watch_path: &Path) -> Option<WatchEvent> {
        if event.paths.is_empty() {
            return None;
        }

        let path = &event.paths[0];

        // Only process files within our watch directory
        if !path.starts_with(watch_path) {
            return None;
        }

        let event_kind = match event.kind {
            EventKind::Create(_) => WatchEventKind::Created,
            EventKind::Modify(_) => WatchEventKind::Modified,
            EventKind::Remove(_) => WatchEventKind::Deleted,
            _ => return None, // Ignore other event types
        };

        Some(WatchEvent::FileChanged {
            path: path.clone(),
            event_kind,
        })
    }

    /// Check if a file should be indexed based on patterns
    fn should_index_file(path: &Path, patterns: &[String]) -> bool {
        // Skip directories
        if path.is_dir() {
            return false;
        }

        // If no patterns specified, index all files
        if patterns.is_empty() {
            return true;
        }

        // Check against patterns
        for pattern in patterns {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                if glob_pattern.matches_path(path) {
                    return true;
                }
            }
        }

        false
    }

    /// Convert WatchEvent to IndexUpdate using TreeSitterIndexer
    fn convert_to_index_update(
        watch_event: WatchEvent,
        patterns: &[String],
    ) -> Option<IndexUpdate> {
        match watch_event {
            WatchEvent::FileChanged { path, event_kind } => {
                // Create a temporary indexer for symbol extraction
                let mut indexer = TreeSitterIndexer::with_options(false, true); // non-verbose, respect gitignore
                if indexer.initialize_sync().is_err() {
                    return None;
                }

                match event_kind {
                    WatchEventKind::Created | WatchEventKind::Modified => {
                        // Extract symbols from the file
                        match indexer.reindex_file(&path, patterns) {
                            Ok(symbols) => {
                                if event_kind == WatchEventKind::Created {
                                    Some(IndexUpdate::Added { file: path, symbols })
                                } else {
                                    Some(IndexUpdate::Modified { file: path, symbols })
                                }
                            }
                            Err(_) => None, // Failed to extract symbols
                        }
                    }
                    WatchEventKind::Deleted => {
                        // For deleted files, we don't need to extract symbols
                        Some(IndexUpdate::Removed { 
                            file: path, 
                            symbol_count: 0  // We don't know the count of removed symbols
                        })
                    }
                    WatchEventKind::Renamed { from: _, to } => {
                        // Treat rename as create new + delete old
                        // For simplicity, just handle the "to" path as a creation
                        match indexer.reindex_file(&to, patterns) {
                            Ok(symbols) => Some(IndexUpdate::Added { file: to, symbols }),
                            Err(_) => None,
                        }
                    }
                }
            }
            WatchEvent::BatchUpdate { events } => {
                // For batch updates, convert the first event only for simplicity
                // In a real implementation, you might want to process all events
                if let Some(first_event) = events.first() {
                    Self::convert_to_index_update(first_event.clone(), patterns)
                } else {
                    None
                }
            }
        }
    }
}

impl std::fmt::Debug for FileWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWatcher")
            .field("debouncer", &self.debouncer)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_event_debouncer() {
        let mut debouncer = EventDebouncer::new(50); // 50ms debounce

        let path = PathBuf::from("/test/file.rs");
        let event = WatchEvent::FileChanged {
            path: path.clone(),
            event_kind: WatchEventKind::Modified,
        };

        // First event should be stored, not returned immediately
        let ready = debouncer.add_event(event.clone());
        assert!(ready.is_empty());

        // Wait for debounce period
        thread::sleep(Duration::from_millis(60));

        // Another event should return the previous one
        let ready = debouncer.add_event(event);
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_should_index_file() {
        let patterns = vec!["**/*.rs".to_string(), "**/*.ts".to_string()];

        assert!(FileWatcher::should_index_file(
            &PathBuf::from("src/main.rs"),
            &patterns
        ));
        assert!(FileWatcher::should_index_file(
            &PathBuf::from("lib/utils.ts"),
            &patterns
        ));
        assert!(!FileWatcher::should_index_file(
            &PathBuf::from("README.md"),
            &patterns
        ));

        // Empty patterns should match all files
        assert!(FileWatcher::should_index_file(
            &PathBuf::from("README.md"),
            &[]
        ));
    }

    #[test]
    fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = vec!["**/*.rs".to_string()];

        let watcher = FileWatcher::new(temp_dir.path(), patterns, Some(100));
        assert!(watcher.is_ok());
    }
}