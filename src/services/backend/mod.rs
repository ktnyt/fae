//! 検索バックエンドモジュール
//! 
//! このモジュールは異なる検索ツール（ripgrep、ag、独自実装）を統一インターフェースで
//! 扱うための抽象化レイヤーを提供します。

use async_trait::async_trait;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

pub mod ripgrep;
pub mod ag;
pub mod native;

/// 検索結果を表す構造体
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    /// ファイルパス
    pub filename: String,
    /// 行番号（1から開始）
    pub line_number: u32,
    /// バイトオフセット
    pub byte_offset: u32,
    /// マッチした行の内容
    pub content: String,
}

/// 検索バックエンドの種類
#[derive(Debug, Clone, PartialEq)]
pub enum BackendType {
    /// ripgrep（最優先）
    Ripgrep,
    /// the_silver_searcher (ag)
    Ag,
    /// 独自実装（フォールバック）
    Native,
}

impl BackendType {
    /// 利用可能なバックエンドを優先順位順で取得
    pub fn available_backends() -> Vec<BackendType> {
        vec![
            BackendType::Ripgrep,
            BackendType::Ag,
            BackendType::Native,
        ]
    }
    
    /// バックエンドの名前を取得
    pub fn name(&self) -> &'static str {
        match self {
            BackendType::Ripgrep => "ripgrep",
            BackendType::Ag => "ag",
            BackendType::Native => "native",
        }
    }
    
    /// バックエンドの説明を取得
    pub fn description(&self) -> &'static str {
        match self {
            BackendType::Ripgrep => "ripgrep - Fast line-oriented search tool",
            BackendType::Ag => "the_silver_searcher - A code searching tool similar to ack",
            BackendType::Native => "Native Rust implementation - Built-in fallback search",
        }
    }
}

/// 検索バックエンドの共通インターフェース
#[async_trait]
pub trait SearchBackend: Send + Sync {
    /// バックエンドの種類を取得
    fn backend_type(&self) -> BackendType;
    
    /// バックエンドが利用可能かチェック
    async fn is_available(&self) -> bool;
    
    /// リテラル検索を実行
    /// 
    /// # Arguments
    /// * `query` - 検索クエリ（リテラル文字列）
    /// * `search_root` - 検索対象のルートディレクトリ
    /// * `cancellation_token` - キャンセレーショントークン
    /// * `result_callback` - 検索結果のコールバック関数
    /// 
    /// # Returns
    /// 検索結果の総数
    async fn search_literal<F>(
        &self,
        query: &str,
        search_root: &PathBuf,
        cancellation_token: CancellationToken,
        result_callback: F,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(SearchMatch) + Send + Sync;
}

/// 具象的な検索バックエンドenum
#[derive(Debug)]
pub enum SearchBackendImpl {
    Ripgrep(ripgrep::RipgrepBackend),
    Ag(ag::AgBackend),
    Native(native::NativeBackend),
}

impl SearchBackendImpl {
    /// 利用可能な最適なバックエンドを自動選択
    pub async fn create_best_available() -> Self {
        for backend_type in BackendType::available_backends() {
            let backend = Self::create(backend_type);
            if backend.is_available().await {
                log::info!("Selected search backend: {}", backend.backend_type().name());
                return backend;
            }
        }
        
        // フォールバックとしてNativeを使用（常に利用可能）
        log::warn!("No external search tools available, using native implementation");
        Self::create(BackendType::Native)
    }
    
    /// 指定されたバックエンドを作成
    pub fn create(backend_type: BackendType) -> Self {
        match backend_type {
            BackendType::Ripgrep => Self::Ripgrep(ripgrep::RipgrepBackend::new()),
            BackendType::Ag => Self::Ag(ag::AgBackend::new()),
            BackendType::Native => Self::Native(native::NativeBackend::new()),
        }
    }
    
    /// 利用可能なバックエンドの一覧を取得
    pub async fn list_available_backends() -> Vec<(BackendType, bool)> {
        let mut backends = Vec::new();
        
        for backend_type in BackendType::available_backends() {
            let backend = Self::create(backend_type.clone());
            let available = backend.is_available().await;
            backends.push((backend_type, available));
        }
        
        backends
    }
}

#[async_trait]
impl SearchBackend for SearchBackendImpl {
    fn backend_type(&self) -> BackendType {
        match self {
            Self::Ripgrep(backend) => backend.backend_type(),
            Self::Ag(backend) => backend.backend_type(),
            Self::Native(backend) => backend.backend_type(),
        }
    }
    
    async fn is_available(&self) -> bool {
        match self {
            Self::Ripgrep(backend) => backend.is_available().await,
            Self::Ag(backend) => backend.is_available().await,
            Self::Native(backend) => backend.is_available().await,
        }
    }
    
    async fn search_literal<F>(
        &self,
        query: &str,
        search_root: &PathBuf,
        cancellation_token: CancellationToken,
        result_callback: F,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(SearchMatch) + Send + Sync,
    {
        match self {
            Self::Ripgrep(backend) => backend.search_literal(query, search_root, cancellation_token, result_callback).await,
            Self::Ag(backend) => backend.search_literal(query, search_root, cancellation_token, result_callback).await,
            Self::Native(backend) => backend.search_literal(query, search_root, cancellation_token, result_callback).await,
        }
    }
}

/// 後方互換性のためのエイリアス
pub type BackendFactory = SearchBackendImpl;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_properties() {
        let ripgrep = BackendType::Ripgrep;
        assert_eq!(ripgrep.name(), "ripgrep");
        assert!(ripgrep.description().contains("ripgrep"));
        
        let ag = BackendType::Ag;
        assert_eq!(ag.name(), "ag");
        assert!(ag.description().contains("silver"));
        
        let native = BackendType::Native;
        assert_eq!(native.name(), "native");
        assert!(native.description().contains("Native"));
    }
    
    #[test]
    fn test_available_backends_order() {
        let backends = BackendType::available_backends();
        assert_eq!(backends[0], BackendType::Ripgrep);
        assert_eq!(backends[1], BackendType::Ag);
        assert_eq!(backends[2], BackendType::Native);
    }
    
    #[test]
    fn test_search_match_creation() {
        let search_match = SearchMatch {
            filename: "test.rs".to_string(),
            line_number: 42,
            byte_offset: 1337,
            content: "fn test() {}".to_string(),
        };
        
        assert_eq!(search_match.filename, "test.rs");
        assert_eq!(search_match.line_number, 42);
        assert_eq!(search_match.byte_offset, 1337);
        assert_eq!(search_match.content, "fn test() {}");
    }
    
    #[tokio::test]
    async fn test_backend_factory_creation() {
        // 各バックエンドの作成をテスト
        let ripgrep = SearchBackendImpl::create(BackendType::Ripgrep);
        assert_eq!(ripgrep.backend_type(), BackendType::Ripgrep);
        
        let ag = SearchBackendImpl::create(BackendType::Ag);
        assert_eq!(ag.backend_type(), BackendType::Ag);
        
        let native = SearchBackendImpl::create(BackendType::Native);
        assert_eq!(native.backend_type(), BackendType::Native);
    }
    
    #[tokio::test]
    async fn test_list_available_backends() {
        let backends = SearchBackendImpl::list_available_backends().await;
        assert_eq!(backends.len(), 3);
        
        // Nativeは常に利用可能
        let native_entry = backends.iter().find(|(bt, _)| *bt == BackendType::Native);
        assert!(native_entry.is_some());
        assert!(native_entry.unwrap().1); // available = true
    }
}