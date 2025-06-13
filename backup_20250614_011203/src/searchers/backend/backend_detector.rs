use super::traits::ExternalSearchBackend;
use super::{RipgrepBackend, AgBackend};
use crate::searchers::ContentSearcher;
use crate::types::SearchResult;
use anyhow::Result;
use std::path::Path;
use log::{debug, info, warn};

/// バックエンド検出とコンテンツ検索統合
pub struct BackendDetector {
    available_backends: Vec<Box<dyn ExternalSearchBackend>>,
    fallback_searcher: Option<ContentSearcher>,
}

impl BackendDetector {
    /// 新しいBackendDetectorを作成
    pub fn new(project_root: &Path) -> Result<Self> {
        let mut available_backends: Vec<Box<dyn ExternalSearchBackend>> = Vec::new();
        
        // 利用可能なバックエンドを検出
        let rg_backend = RipgrepBackend::new();
        if rg_backend.is_available() {
            info!("Detected ripgrep backend");
            available_backends.push(Box::new(rg_backend));
        }
        
        let ag_backend = AgBackend::new();
        if ag_backend.is_available() {
            info!("Detected ag backend");
            available_backends.push(Box::new(ag_backend));
        }
        
        // 優先度順でソート（高い順）
        available_backends.sort_by(|a, b| b.priority().cmp(&a.priority()));
        
        // フォールバック用の内蔵検索エンジンを準備
        let fallback_searcher = if available_backends.is_empty() {
            info!("No external backends available, using built-in search");
            Some(ContentSearcher::new(project_root.to_path_buf())?)
        } else {
            debug!("External backends available: {}", 
                   available_backends.iter().map(|b| b.name()).collect::<Vec<_>>().join(", "));
            None
        };
        
        Ok(Self {
            available_backends,
            fallback_searcher,
        })
    }
    
    /// 最適なバックエンドでコンテンツ検索を実行
    pub fn search_content(&self, project_root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        // 優先度の高いバックエンドから順に試行
        for backend in &self.available_backends {
            match backend.search_content(project_root, query) {
                Ok(results) => {
                    debug!("Used backend: {} (found {} results)", backend.name(), results.len());
                    // 各バックエンドの自然な順序を保持
                    return Ok(results);
                }
                Err(err) => {
                    warn!("{} backend failed: {}", backend.name(), err);
                    continue;
                }
            }
        }
        
        // 全ての外部バックエンドが失敗した場合はフォールバック
        if let Some(fallback) = &self.fallback_searcher {
            debug!("Used backend: fallback (built-in)");
            return fallback.search(query, 1000); // デフォルト制限
        }
        
        // フォールバックも利用できない場合
        Err(anyhow::anyhow!("No search backends available"))
    }
    
    /// 最適なバックエンドで正規表現検索を実行
    pub fn search_regex(&self, project_root: &Path, pattern: &str) -> Result<Vec<SearchResult>> {
        // 優先度の高いバックエンドから順に試行
        for backend in &self.available_backends {
            match backend.search_regex(project_root, pattern) {
                Ok(results) => {
                    debug!("Used backend: {} for regex search (found {} results)", backend.name(), results.len());
                    // 各バックエンドの自然な順序を保持
                    return Ok(results);
                }
                Err(err) => {
                    warn!("{} backend regex search failed: {}", backend.name(), err);
                    continue;
                }
            }
        }
        
        // 全ての外部バックエンドが失敗した場合はフォールバック
        if let Some(fallback) = &self.fallback_searcher {
            debug!("Used backend: fallback (built-in) for regex search");
            // フォールバックでは通常のコンテンツ検索を使用（将来的にregex crateを統合予定）
            return fallback.search(pattern, 1000); // デフォルト制限
        }
        
        // フォールバックも利用できない場合
        Err(anyhow::anyhow!("No search backends available for regex"))
    }
    
    /// 利用可能なバックエンドの一覧を取得
    pub fn available_backends(&self) -> Vec<&str> {
        let mut backends: Vec<&str> = self.available_backends
            .iter()
            .map(|backend| backend.name())
            .collect();
        
        if self.fallback_searcher.is_some() {
            backends.push("built-in");
        }
        
        backends
    }
    
    /// 最優先バックエンドの名前を取得
    pub fn primary_backend(&self) -> &str {
        self.available_backends
            .first()
            .map(|backend| backend.name())
            .unwrap_or("built-in")
    }
}