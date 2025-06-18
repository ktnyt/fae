use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Notify Library Test ===");
    
    // Create channel for receiving events
    let (tx, rx) = mpsc::channel();
    
    // Create watcher with same config as fae
    let config = Config::default()
        .with_poll_interval(Duration::from_millis(500))
        .with_compare_contents(true);
    
    println!("Creating watcher with config:");
    println!("  Poll interval: 500ms");
    println!("  Compare contents: true");
    
    let mut watcher = RecommendedWatcher::new(tx, config)
        .map_err(|e| format!("Failed to create file watcher: {}", e))?;
    
    // Watch current directory (same as fae)
    let watch_path = ".";
    println!("Starting to watch: {}", watch_path);
    
    watcher
        .watch(Path::new(watch_path), RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to start watching: {}", e))?;
    
    println!("Watcher started successfully!");
    println!("Monitoring for file changes... (Press Ctrl+C to exit)");
    println!("Try creating, modifying, or deleting files to see events.");
    println!();
    
    // Process events
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event_result) => {
                match event_result {
                    Ok(event) => {
                        process_event(event);
                    }
                    Err(e) => {
                        println!("❌ Watch error: {}", e);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events received, continue monitoring
                print!(".");
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                println!("Channel disconnected");
                break;
            }
        }
    }
    
    Ok(())
}

fn process_event(event: Event) {
    println!("\n🔔 Event received:");
    println!("  Kind: {:?}", event.kind);
    println!("  Paths: {:?}", event.paths);
    
    for path in &event.paths {
        if path.is_dir() {
            println!("  📁 Directory: {}", path.display());
            continue;
        }
        
        let file_path_str = path.to_string_lossy();
        
        match event.kind {
            EventKind::Create(_) => {
                println!("  ✅ File created: {}", file_path_str);
            }
            EventKind::Modify(_) => {
                println!("  ✏️  File modified: {}", file_path_str);
            }
            EventKind::Remove(_) => {
                println!("  🗑️  File deleted: {}", file_path_str);
            }
            EventKind::Access(_) => {
                println!("  👁️  File accessed: {}", file_path_str);
            }
            _ => {
                println!("  ❓ Other event: {:?} for {}", event.kind, file_path_str);
            }
        }
    }
    println!();
}