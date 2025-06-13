pub mod worker;
pub mod message;
pub mod protocol;
pub mod tui_worker;
pub mod simple_tui;
pub mod search_handler;
pub mod base_searcher;
pub mod content_searcher;

pub use worker::{Worker, WorkerHandle, WorkerManager};
pub use message::{Message, MessageBus};
pub use protocol::{
    WorkerMessage, TuiMessage, SearchHandlerMessage, SearchQueryMessage, 
    SearchResultMessage, WatcherMessage
};
pub use tui_worker::TuiWorker;
pub use simple_tui::SimpleTuiWorker;
pub use search_handler::SearchHandler;
pub use content_searcher::ContentSearcher;