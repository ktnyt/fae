pub mod types;
pub mod searcher;
pub mod indexer;
pub mod tui;
pub mod parsers;
pub mod filters;
pub mod file_watcher;
pub mod cache_manager;

// New event-based architecture modules
pub mod backend;
pub mod tui_state;
pub mod tui_simulator;

// 公開API
pub use types::*;
pub use searcher::*;
pub use indexer::*;
pub use file_watcher::*;
pub use tui::{run_tui, run_tui_with_watch};

// New APIs for testing and simulation
pub use backend::{SearchBackend, BackendEvent, UserCommand};
pub use tui_state::{TuiState, TuiInput, TuiAction};
pub use tui_simulator::TuiSimulator;