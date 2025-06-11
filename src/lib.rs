pub mod cache_manager;
pub mod file_watcher;
pub mod filters;
pub mod indexer;
pub mod mode;
pub mod parsers;
pub mod searcher;
pub mod tui;
pub mod types;

// New event-based architecture modules
pub mod backend;
pub mod tui_simulator;
pub mod tui_state;

// 公開API
pub use file_watcher::*;
pub use indexer::*;
pub use searcher::*;
pub use tui::{run_tui, run_tui_with_watch};
pub use types::*;

// New APIs for testing and simulation
pub use backend::{BackendEvent, SearchBackend, UserCommand};
pub use tui_simulator::TuiSimulator;
pub use tui_state::{TuiAction, TuiInput, TuiState};
