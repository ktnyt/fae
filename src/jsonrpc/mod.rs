//! JSON-RPC 2.0 Implementation
//! 
//! このモジュールは、物理プロセス分離による疎結合アーキテクチャを実現する
//! JSON-RPC 2.0の完全な実装を提供します。
//! 
//! ## 主要コンポーネント
//! 
//! - [`types`]: JSON-RPC 2.0メッセージ型定義
//! - [`base`]: JsonRpcBase - 双方向通信の中核実装
//! 
//! ## 設計哲学
//! 
//! "どうあがいても疎結合にならざるを得ない" - 物理的なプロセス境界により、
//! 密結合を構造的に不可能にするアーキテクチャを採用しています。
//! 
//! ## 使用例
//! 
//! ```rust,no_run
//! use fae::jsonrpc::{JsonRpcBase, MainLoopHandler};
//! 
//! // サーバー側（stdio通信）
//! let rpc_base = JsonRpcBase::new_stdio().await?;
//! 
//! // クライアント側（子プロセス管理）
//! let rpc_base = JsonRpcBase::spawn("server_binary", &[]).await?;
//! ```

pub mod types;
pub mod base;

// Re-export main types for convenience
pub use types::{Request, Response, Message, ErrorObject, ErrorCode};
pub use base::{JsonRpcBase, MainLoopHandler, RpcResult, RpcError};