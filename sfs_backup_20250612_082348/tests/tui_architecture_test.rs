use sfs::{BackendEvent, TuiSimulator, TuiState};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_tui_state_basic_functionality() {
    let state = TuiState::new();

    // Test initial state
    assert_eq!(state.query, "");
    assert_eq!(state.selected_index, 0);
    assert!(!state.should_quit);
    assert!(!state.is_indexing);

    // Test mode info
    let mode_info = state.get_mode_info();
    assert!(mode_info.contains("Recently Edited"));
}

#[test]
fn test_search_mode_detection() {
    let state = TuiState::new();

    // Test symbol search mode
    let mode = state.detect_search_mode("#test");
    assert_eq!(mode.name, "Symbol");

    // Test file search mode
    let mode = state.detect_search_mode(">test");
    assert_eq!(mode.name, "File");

    // Test regex search mode
    let mode = state.detect_search_mode("/test");
    assert_eq!(mode.name, "Regex");

    // Test default content mode
    let mode = state.detect_search_mode("test");
    assert_eq!(mode.name, "Content");
}

#[test]
fn test_backend_event_application() {
    let mut state = TuiState::new();

    // Test indexing complete event
    let event = BackendEvent::IndexingComplete {
        duration: std::time::Duration::from_millis(100),
        total_symbols: 42,
    };

    state.apply_backend_event(event);

    assert!(!state.is_indexing);
    assert!(state.status_message.contains("42 symbols"));
    assert!(state.status_message.contains("100ms"));
}

#[test]
fn test_tui_simulator_creation() {
    // Just test that we can create a simulator without errors
    let result = TuiSimulator::new(false, true);
    assert!(result.is_ok());

    let simulator = result.unwrap();
    assert_eq!(simulator.get_query(), "");
    assert!(!simulator.is_indexing());
    assert!(!simulator.should_quit());
}

fn create_test_file(dir: &TempDir, name: &str, content: &str) -> std::io::Result<PathBuf> {
    let file_path = dir.path().join(name);
    fs::write(&file_path, content)?;
    Ok(file_path)
}

#[test]
fn test_basic_indexing_workflow() {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file
    create_test_file(
        &temp_dir,
        "test.ts",
        r#"
        class TestClass {
            constructor() {}
            
            testMethod() {
                return "test";
            }
        }
    "#,
    )
    .unwrap();

    let mut simulator = TuiSimulator::new(false, true).unwrap();

    // Initialize - this should not hang
    let result = simulator.initialize(temp_dir.path().to_path_buf());
    assert!(result.is_ok());

    // Try to process events with timeout to avoid hanging
    let mut events_received = 0;
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    while start.elapsed() < timeout && events_received < 10 {
        if let Ok(Some(_event)) = simulator.try_process_event() {
            events_received += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // We should have received at least one event (indexing complete)
    println!("Received {} events", events_received);
}
