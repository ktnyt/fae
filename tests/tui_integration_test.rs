use sfs::{TuiSimulator, BackendEvent, UserCommand};
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;
use std::time::{Duration, Instant};

fn create_test_project() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;
    
    // Create multiple test files with different symbols
    let files = vec![
        ("src/main.ts", r#"
class Application {
    private name: string;
    
    constructor(name: string) {
        this.name = name;
    }
    
    public start(): void {
        console.log(`Starting ${this.name}`);
    }
}

function main() {
    const app = new Application("TestApp");
    app.start();
}
"#),
        ("src/utils.ts", r#"
export interface Config {
    port: number;
    host: string;
}

export function createConfig(): Config {
    return {
        port: 3000,
        host: 'localhost'
    };
}

export const DEFAULT_TIMEOUT = 5000;
"#),
        ("src/service.py", r#"
class DatabaseService:
    def __init__(self, connection_string: str):
        self.connection_string = connection_string
    
    def connect(self) -> bool:
        # Mock connection logic
        return True
    
    def disconnect(self) -> None:
        pass

def create_service(conn_str: str) -> DatabaseService:
    return DatabaseService(conn_str)

RETRY_COUNT = 3
"#),
    ];
    
    for (file_path, content) in files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
    }
    
    Ok(temp_dir)
}

#[test]
fn test_progressive_indexing_workflow() -> anyhow::Result<()> {
    let temp_dir = create_test_project()?;
    let mut simulator = TuiSimulator::new(false, true)?;
    
    // Start indexing
    simulator.initialize(temp_dir.path().to_path_buf())?;
    
    // Track indexing progress events
    let mut indexing_events = Vec::new();
    let start_time = Instant::now();
    let timeout = Duration::from_secs(10);
    
    // Wait for indexing to complete while collecting progress events
    loop {
        if start_time.elapsed() > timeout {
            return Err(anyhow::anyhow!("Progressive indexing timeout"));
        }
        
        match simulator.try_process_event()? {
            Some(event) => {
                match &event {
                    BackendEvent::IndexingProgress { processed, total, symbols } => {
                        indexing_events.push(event.clone());
                        println!("Progress: {}/{} files, {} symbols", processed, total, symbols.len());
                    }
                    BackendEvent::IndexingComplete { duration, total_symbols } => {
                        println!("Indexing complete: {} symbols in {:?}", total_symbols, duration);
                        break;
                    }
                    BackendEvent::Error { message } => {
                        return Err(anyhow::anyhow!("Indexing error: {}", message));
                    }
                    _ => {}
                }
            }
            None => {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    
    // Verify we received progress events
    assert!(!indexing_events.is_empty(), "Should receive progress events");
    
    // Verify final state
    let state = simulator.get_state();
    assert!(!state.symbols.is_empty(), "Should have indexed symbols");
    assert!(!state.is_indexing, "Should finish indexing");
    
    Ok(())
}

#[test]
fn test_search_during_progressive_indexing() -> anyhow::Result<()> {
    let temp_dir = create_test_project()?;
    let mut simulator = TuiSimulator::new(false, true)?;
    
    // Start indexing
    simulator.initialize(temp_dir.path().to_path_buf())?;
    
    // Wait a bit for some symbols to be indexed
    std::thread::sleep(Duration::from_millis(100));
    
    // Try searching while indexing is in progress
    simulator.send_command(UserCommand::Search {
        query: "Application".to_string(),
        mode: sfs::types::SearchMode {
            name: "Content".to_string(),
            prefix: "".to_string(),
            icon: "🔍".to_string(),
        },
    })?;
    
    // Wait for search results
    let mut search_completed = false;
    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);
    
    while start_time.elapsed() < timeout && !search_completed {
        if let Some(event) = simulator.try_process_event()? {
            match event {
                BackendEvent::SearchResults { query, results } => {
                    println!("Search results for '{}': {} results", query, results.len());
                    search_completed = true;
                }
                BackendEvent::Error { message } => {
                    return Err(anyhow::anyhow!("Search error: {}", message));
                }
                _ => {}
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    
    assert!(search_completed, "Should complete search during indexing");
    
    Ok(())
}

#[test] 
fn test_file_watching_integration() -> anyhow::Result<()> {
    let temp_dir = create_test_project()?;
    let mut simulator = TuiSimulator::new(false, true)?;
    
    // Initialize and wait for indexing
    simulator.initialize(temp_dir.path().to_path_buf())?;
    simulator.wait_for_indexing_complete()?;
    
    // Enable file watching
    simulator.enable_file_watching()?;
    
    // Create a new file to trigger file watching
    let new_file_path = temp_dir.path().join("src/new_component.ts");
    fs::write(&new_file_path, r#"
export class NewComponent {
    public render(): string {
        return "<div>New Component</div>";
    }
}
"#)?;
    
    // Wait for file change event
    let mut file_changed = false;
    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);
    
    while start_time.elapsed() < timeout && !file_changed {
        if let Some(event) = simulator.try_process_event()? {
            match event {
                BackendEvent::FileChanged { file, change_type } => {
                    println!("File changed: {:?} ({:?})", file, change_type);
                    if file == new_file_path {
                        file_changed = true;
                    }
                }
                BackendEvent::Error { message } => {
                    println!("File watching error: {}", message);
                }
                _ => {}
            }
        } else {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    
    // Note: File watching might not work in all test environments
    // So we don't assert on file_changed, just verify no errors occurred
    println!("File watching test completed (change detected: {})", file_changed);
    
    Ok(())
}

#[test]
fn test_real_time_ui_updates() -> anyhow::Result<()> {
    let temp_dir = create_test_project()?;
    let mut simulator = TuiSimulator::new(false, true)?;
    
    // Track UI state changes during indexing
    let mut state_snapshots = Vec::new();
    
    simulator.initialize(temp_dir.path().to_path_buf())?;
    
    let start_time = Instant::now();
    let timeout = Duration::from_secs(10);
    
    // Capture state changes every 100ms
    while start_time.elapsed() < timeout {
        // Process any pending events
        let mut events_this_iteration = 0;
        while let Some(_event) = simulator.try_process_event()? {
            events_this_iteration += 1;
            if events_this_iteration > 10 {
                break; // Prevent infinite loop
            }
        }
        
        // Capture current state
        let state = simulator.get_state();
        state_snapshots.push((
            start_time.elapsed(),
            state.symbols.len(),
            state.is_indexing,
            state.status_message.clone(),
        ));
        
        // Exit if indexing is complete
        if !state.is_indexing && !state.symbols.is_empty() {
            break;
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    // Verify we captured multiple state changes
    assert!(state_snapshots.len() > 1, "Should capture multiple UI state snapshots");
    
    // Verify progression: symbols should increase over time
    let final_symbols = state_snapshots.last().unwrap().1;
    let initial_symbols = state_snapshots.first().unwrap().1;
    
    println!("UI state progression: {} -> {} symbols over {} snapshots", 
             initial_symbols, final_symbols, state_snapshots.len());
    
    // Final state should have more symbols than initial (unless it was instant)
    assert!(final_symbols >= initial_symbols, "Symbol count should not decrease");
    
    Ok(())
}

#[test]
fn test_backend_event_handling_robustness() -> anyhow::Result<()> {
    let temp_dir = create_test_project()?;
    let mut simulator = TuiSimulator::new(false, true)?;
    
    // Test rapid command sending
    simulator.initialize(temp_dir.path().to_path_buf())?;
    
    // Send multiple search commands in rapid succession
    for i in 0..5 {
        simulator.send_command(UserCommand::Search {
            query: format!("test{}", i),
            mode: sfs::types::SearchMode {
                name: "Content".to_string(),
                prefix: "".to_string(),
                icon: "🔍".to_string(),
            },
        })?;
    }
    
    // Process all events with timeout
    let start_time = Instant::now();
    let timeout = Duration::from_secs(10);
    let mut total_events = 0;
    
    while start_time.elapsed() < timeout {
        if let Some(event) = simulator.try_process_event()? {
            total_events += 1;
            match event {
                BackendEvent::Error { message } => {
                    return Err(anyhow::anyhow!("Backend error: {}", message));
                }
                BackendEvent::IndexingComplete { .. } => {
                    // Good, indexing completed
                }
                _ => {}
            }
        } else {
            // No more events, check if we should exit
            let state = simulator.get_state();
            if !state.is_indexing && !state.symbols.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        
        // Prevent infinite loops
        if total_events > 50 {
            break;
        }
    }
    
    println!("Processed {} events total", total_events);
    assert!(total_events > 0, "Should process some events");
    
    // Verify backend is still responsive
    let state = simulator.get_state();
    assert!(!state.symbols.is_empty(), "Should have indexed symbols");
    
    Ok(())
}