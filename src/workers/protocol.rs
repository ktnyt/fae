use serde::{Deserialize, Serialize};

/// TUI-SearchHandler-BaseSearcher間の通信プロトコル定義
/// 設計書(.claude/tuidesign.md)に基づく実装

/// TUI → SearchHandler メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "payload")]
pub enum TuiMessage {
    /// method: user/query
    #[serde(rename = "user/query")]
    UserQuery { query: String },
}

/// SearchHandler → TUI メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "payload")]
pub enum SearchHandlerMessage {
    /// method: search/clear
    #[serde(rename = "search/clear")]
    SearchClear,

    /// method: search/match
    #[serde(rename = "search/match")]
    SearchMatch {
        filename: String,
        line: u32,
        column: u32,
        content: String,
    },

    /// method: index/progress (SymbolSearcher → TUI via SearchHandler)
    #[serde(rename = "index/progress")]
    IndexProgress {
        indexed_files: u32,
        total_files: u32,
        symbols: u32,
        elapsed: u64, // milliseconds
    },

    /// method: index/update (SymbolSearcher → TUI via SearchHandler)
    #[serde(rename = "index/update")]
    IndexUpdate {
        filename: String,
        symbols: u32,
        elapsed: u64, // milliseconds
    },
}

/// SearchHandler → BaseSearcher メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "payload")]
pub enum SearchQueryMessage {
    /// method: user/query
    #[serde(rename = "user/query")]
    UserQuery { query: String },
}

/// BaseSearcher → SearchHandler メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "payload")]
pub enum SearchResultMessage {
    /// method: search/clear
    #[serde(rename = "search/clear")]
    SearchClear,

    /// method: search/match
    #[serde(rename = "search/match")]
    SearchMatch {
        filename: String,
        line: u32,
        column: u32,
        content: String,
    },
}

/// Watcher → SymbolSearcher メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "payload")]
pub enum WatcherMessage {
    /// method: file/create
    #[serde(rename = "file/create")]
    FileCreate { filename: String },

    /// method: file/update
    #[serde(rename = "file/update")]
    FileUpdate { filename: String },

    /// method: file/delete
    #[serde(rename = "file/delete")]
    FileDelete { filename: String },
}

/// 統一されたワーカーメッセージ型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum WorkerMessage {
    #[serde(rename = "tui")]
    Tui(TuiMessage),

    #[serde(rename = "search_handler")]
    SearchHandler(SearchHandlerMessage),

    #[serde(rename = "search_query")]
    SearchQuery(SearchQueryMessage),

    #[serde(rename = "search_result")]
    SearchResult(SearchResultMessage),

    #[serde(rename = "watcher")]
    Watcher(WatcherMessage),
}

impl WorkerMessage {
    /// TUIメッセージを作成
    pub fn tui_query(query: String) -> Self {
        Self::Tui(TuiMessage::UserQuery { query })
    }

    /// SearchHandlerメッセージを作成
    pub fn search_clear() -> Self {
        Self::SearchHandler(SearchHandlerMessage::SearchClear)
    }

    pub fn search_match(filename: String, line: u32, column: u32, content: String) -> Self {
        Self::SearchHandler(SearchHandlerMessage::SearchMatch {
            filename,
            line,
            column,
            content,
        })
    }

    pub fn index_progress(
        indexed_files: u32,
        total_files: u32,
        symbols: u32,
        elapsed: u64,
    ) -> Self {
        Self::SearchHandler(SearchHandlerMessage::IndexProgress {
            indexed_files,
            total_files,
            symbols,
            elapsed,
        })
    }

    pub fn index_update(filename: String, symbols: u32, elapsed: u64) -> Self {
        Self::SearchHandler(SearchHandlerMessage::IndexUpdate {
            filename,
            symbols,
            elapsed,
        })
    }

    /// SearchQuery メッセージを作成
    pub fn search_query(query: String) -> Self {
        Self::SearchQuery(SearchQueryMessage::UserQuery { query })
    }

    /// SearchResult メッセージを作成
    pub fn search_result_clear() -> Self {
        Self::SearchResult(SearchResultMessage::SearchClear)
    }

    pub fn search_result_match(filename: String, line: u32, column: u32, content: String) -> Self {
        Self::SearchResult(SearchResultMessage::SearchMatch {
            filename,
            line,
            column,
            content,
        })
    }

    /// Watcherメッセージを作成
    pub fn file_create(filename: String) -> Self {
        Self::Watcher(WatcherMessage::FileCreate { filename })
    }

    pub fn file_update(filename: String) -> Self {
        Self::Watcher(WatcherMessage::FileUpdate { filename })
    }

    pub fn file_delete(filename: String) -> Self {
        Self::Watcher(WatcherMessage::FileDelete { filename })
    }

    /// メッセージの種類を取得
    pub fn get_type(&self) -> &'static str {
        match self {
            Self::Tui(_) => "tui",
            Self::SearchHandler(_) => "search_handler",
            Self::SearchQuery(_) => "search_query",
            Self::SearchResult(_) => "search_result",
            Self::Watcher(_) => "watcher",
        }
    }

    /// メッセージのメソッド名を取得
    pub fn get_method(&self) -> &'static str {
        match self {
            Self::Tui(TuiMessage::UserQuery { .. }) => "user/query",
            Self::SearchHandler(SearchHandlerMessage::SearchClear) => "search/clear",
            Self::SearchHandler(SearchHandlerMessage::SearchMatch { .. }) => "search/match",
            Self::SearchHandler(SearchHandlerMessage::IndexProgress { .. }) => "index/progress",
            Self::SearchHandler(SearchHandlerMessage::IndexUpdate { .. }) => "index/update",
            Self::SearchQuery(SearchQueryMessage::UserQuery { .. }) => "user/query",
            Self::SearchResult(SearchResultMessage::SearchClear) => "search/clear",
            Self::SearchResult(SearchResultMessage::SearchMatch { .. }) => "search/match",
            Self::Watcher(WatcherMessage::FileCreate { .. }) => "file/create",
            Self::Watcher(WatcherMessage::FileUpdate { .. }) => "file/update",
            Self::Watcher(WatcherMessage::FileDelete { .. }) => "file/delete",
        }
    }
}

/// プロトコルのヘルパー関数
impl From<WorkerMessage> for crate::workers::Message {
    fn from(worker_msg: WorkerMessage) -> Self {
        crate::workers::Message::new(
            worker_msg.get_method(),
            serde_json::to_value(worker_msg).unwrap_or_default(),
        )
    }
}

impl TryFrom<crate::workers::Message> for WorkerMessage {
    type Error = serde_json::Error;

    fn try_from(msg: crate::workers::Message) -> Result<Self, Self::Error> {
        serde_json::from_value(msg.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_message_serialization() {
        let msg = WorkerMessage::tui_query("hello world".to_string());
        let json = serde_json::to_value(&msg).unwrap();
        
        assert_eq!(json["type"], "tui");
        assert_eq!(json["message"]["method"], "user/query");
        assert_eq!(json["message"]["payload"]["query"], "hello world");
    }

    #[test]
    fn test_search_handler_message_serialization() {
        let msg = WorkerMessage::search_match(
            "src/main.rs".to_string(),
            42,
            10,
            "fn main() {".to_string(),
        );
        let json = serde_json::to_value(&msg).unwrap();
        
        assert_eq!(json["type"], "search_handler");
        assert_eq!(json["message"]["method"], "search/match");
        assert_eq!(json["message"]["payload"]["filename"], "src/main.rs");
        assert_eq!(json["message"]["payload"]["line"], 42);
        assert_eq!(json["message"]["payload"]["column"], 10);
        assert_eq!(json["message"]["payload"]["content"], "fn main() {");
    }

    #[test]
    fn test_watcher_message_serialization() {
        let msg = WorkerMessage::file_update("src/lib.rs".to_string());
        let json = serde_json::to_value(&msg).unwrap();
        
        assert_eq!(json["type"], "watcher");
        assert_eq!(json["message"]["method"], "file/update");
        assert_eq!(json["message"]["payload"]["filename"], "src/lib.rs");
    }

    #[test]
    fn test_message_conversion() {
        let worker_msg = WorkerMessage::index_progress(50, 100, 1500, 2000);
        let msg: crate::workers::Message = worker_msg.clone().into();
        
        assert_eq!(msg.method, "index/progress");
        
        let converted_back = WorkerMessage::try_from(msg).unwrap();
        match converted_back {
            WorkerMessage::SearchHandler(SearchHandlerMessage::IndexProgress {
                indexed_files,
                total_files,
                symbols,
                elapsed,
            }) => {
                assert_eq!(indexed_files, 50);
                assert_eq!(total_files, 100);
                assert_eq!(symbols, 1500);
                assert_eq!(elapsed, 2000);
            }
            _ => panic!("Conversion failed"),
        }
    }

    #[test]
    fn test_get_method() {
        assert_eq!(WorkerMessage::tui_query("test".to_string()).get_method(), "user/query");
        assert_eq!(
            WorkerMessage::search_match("file".to_string(), 1, 2, "content".to_string()).get_method(),
            "search/match"
        );
        assert_eq!(
            WorkerMessage::index_progress(1, 2, 3, 4).get_method(),
            "index/progress"
        );
        assert_eq!(
            WorkerMessage::file_create("file".to_string()).get_method(),
            "file/create"
        );
    }
}