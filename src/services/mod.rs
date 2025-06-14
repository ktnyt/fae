//! Services module
//! 
//! このモジュールには、faeプロジェクトの様々な検索・処理サービスが含まれています。
//! 各サービスはJSON-RPCハンドラーとして実装され、マイクロサービスアーキテクチャの
//! 一部として独立して動作できるよう設計されています。

pub mod backend;
pub mod literal_search;
pub mod service_factory;

// 将来追加予定のサービス
// pub mod symbol_search;      // シンボル検索サービス
// pub mod file_search;        // ファイル名検索サービス  
// pub mod regex_search;       // 正規表現検索サービス
// pub mod git_search;         // Git統合検索サービス

// Re-exports for convenience
pub use literal_search::LiteralSearchHandler;
pub use service_factory::{ServiceFactory, ServiceType};