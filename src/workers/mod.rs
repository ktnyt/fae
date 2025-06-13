pub mod worker;
pub mod message;
pub mod protocol;
pub mod simple_tui;
pub mod search_router_worker;
pub mod content_search_worker;

pub use worker::{Worker, WorkerHandle, WorkerManager};
pub use message::{Message, MessageBus};
pub use protocol::{
    WorkerMessage, TuiMessage, SearchRouterMessage, SearchQueryMessage, 
    SearchResultMessage, WatcherMessage
};
pub use simple_tui::SimpleTuiWorker;
pub use search_router_worker::SearchRouterWorker;
pub use content_search_worker::ContentSearchWorker;