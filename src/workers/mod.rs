//! JSON-RPC Workers Module
//! 
//! 物理プロセス分離による疎結合検索ワーカー実装
//! 
//! 各ワーカーは独立したプロセスとして動作し、JSON-RPC経由で通信する。
//! これにより「どうあがいても疎結合にならざるを得ない」アーキテクチャを実現。

pub mod message_types;
pub mod content_search_worker;
pub mod search_router;