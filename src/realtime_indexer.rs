//! リアルタイムファイル監視によるシンボルインデックス更新
//! 
//! notify crateを使用してファイルシステム変更を監視し、
//! 変更されたファイルのみのシンボル情報を選択的に更新する

use crate::cache_manager::CacheManager;
use crate::symbol_index::SymbolMetadata;
use crate::tree_sitter::extract_symbols_from_file;
use anyhow::{Context, Result};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

/// ファイル変更イベントの種類
#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    /// ファイルが作成された
    Created(PathBuf),
    /// ファイルが変更された  
    Modified(PathBuf),
    /// ファイルが削除された
    Removed(PathBuf),
    /// ファイルが移動された（古いパス、新しいパス）
    Moved(PathBuf, PathBuf),
}

/// リアルタイムインデックス更新結果
#[derive(Debug)]
pub struct IndexUpdateResult {
    /// 更新されたファイル数
    pub updated_files: usize,
    /// 追加されたシンボル数
    pub added_symbols: usize,
    /// 削除されたシンボル数
    pub removed_symbols: usize,
    /// 更新にかかった時間
    pub duration: Duration,
}

/// リアルタイムファイル監視とシンボルインデックス更新
pub struct RealtimeIndexer {
    /// notify ファイル監視
    _watcher: RecommendedWatcher,
    /// ファイル変更イベント受信チャンネル
    event_receiver: mpsc::UnboundedReceiver<FileChangeEvent>,
    /// キャッシュマネージャー（共有）
    cache_manager: Arc<Mutex<CacheManager>>,
    /// プロジェクトルート
    #[allow(dead_code)]
    project_root: PathBuf,
    /// 監視対象の拡張子
    watched_extensions: HashSet<String>,
    /// 更新バッチング用のデバウンス時間（ms）
    debounce_ms: u64,
}

impl RealtimeIndexer {
    /// 新しいRealtimeIndexerを作成
    pub fn new(
        project_root: PathBuf,
        cache_manager: Arc<Mutex<CacheManager>>,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        // 監視対象拡張子を設定
        let watched_extensions: HashSet<String> = [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h"
        ].iter().map(|&s| s.to_string()).collect();
        
        // ファイル監視設定
        let config = Config::default()
            .with_poll_interval(Duration::from_millis(100))
            .with_compare_contents(false); // パフォーマンス優先
        
        let sender_for_watcher = event_sender.clone();
        let watched_exts_for_closure = watched_extensions.clone();
        
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                if let Ok(event) = result {
                    if let Some(change_event) = Self::process_notify_event(event, &watched_exts_for_closure) {
                        let _ = sender_for_watcher.send(change_event);
                    }
                }
            },
            config,
        )?;
        
        // プロジェクトルートの監視開始
        watcher.watch(&project_root, RecursiveMode::Recursive)
            .context("Failed to start watching project directory")?;
        
        log::info!("Started file watching for: {}", project_root.display());
        
        Ok(Self {
            _watcher: watcher,
            event_receiver,
            cache_manager,
            project_root,
            watched_extensions,
            debounce_ms: 150, // デフォルト150ms
        })
    }
    
    /// notify::Eventを処理してFileChangeEventに変換
    fn process_notify_event(
        event: Event,
        watched_extensions: &HashSet<String>
    ) -> Option<FileChangeEvent> {
        // パスの拡張子をチェック
        let is_watched_file = |path: &Path| -> bool {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| watched_extensions.contains(ext))
                .unwrap_or(false)
        };
        
        match event.kind {
            EventKind::Create(_) => {
                event.paths.into_iter()
                    .find(|p| is_watched_file(p))
                    .map(FileChangeEvent::Created)
            }
            EventKind::Modify(_) => {
                event.paths.into_iter()
                    .find(|p| is_watched_file(p))
                    .map(FileChangeEvent::Modified)
            }
            EventKind::Remove(_) => {
                event.paths.into_iter()
                    .find(|p| is_watched_file(p))
                    .map(FileChangeEvent::Removed)
            }
            EventKind::Other => {
                // 移動などの特殊ケース
                if event.paths.len() >= 2 {
                    let old_path = &event.paths[0];
                    let new_path = &event.paths[1];
                    if is_watched_file(old_path) || is_watched_file(new_path) {
                        return Some(FileChangeEvent::Moved(old_path.clone(), new_path.clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }
    
    /// ファイル変更イベントループを開始
    pub async fn start_event_loop(&mut self) -> Result<()> {
        let mut pending_changes: HashSet<PathBuf> = HashSet::new();
        let mut last_change_time = SystemTime::now();
        
        loop {
            // デバウンス時間内にイベントをバッチ処理
            let timeout = Duration::from_millis(self.debounce_ms);
            
            tokio::select! {
                // 新しいイベント受信
                Some(event) = self.event_receiver.recv() => {
                    match event {
                        FileChangeEvent::Created(path) | FileChangeEvent::Modified(path) => {
                            pending_changes.insert(path);
                            last_change_time = SystemTime::now();
                        }
                        FileChangeEvent::Removed(path) => {
                            pending_changes.remove(&path);
                            self.remove_file_from_index(&path).await?;
                        }
                        FileChangeEvent::Moved(old_path, new_path) => {
                            pending_changes.remove(&old_path);
                            pending_changes.insert(new_path);
                            self.remove_file_from_index(&old_path).await?;
                            last_change_time = SystemTime::now();
                        }
                    }
                }
                
                // デバウンス時間経過後のバッチ処理
                _ = tokio::time::sleep(timeout) => {
                    if !pending_changes.is_empty() && 
                       last_change_time.elapsed().unwrap_or_default() >= timeout {
                        
                        let files: Vec<PathBuf> = pending_changes.drain().collect();
                        
                        if let Ok(result) = self.update_files_batch(&files).await {
                            log::debug!(
                                "Updated {} files, +{} -{} symbols in {:?}",
                                result.updated_files,
                                result.added_symbols,
                                result.removed_symbols,
                                result.duration
                            );
                        }
                    }
                }
            }
        }
    }
    
    /// 複数ファイルのバッチ更新
    async fn update_files_batch(&self, files: &[PathBuf]) -> Result<IndexUpdateResult> {
        let start_time = SystemTime::now();
        let mut added_symbols = 0;
        let mut removed_symbols = 0;
        let mut updated_files = 0;
        
        for file_path in files {
            if file_path.exists() {
                match self.update_single_file(file_path).await {
                    Ok((added, removed)) => {
                        added_symbols += added;
                        removed_symbols += removed;
                        updated_files += 1;
                    }
                    Err(e) => {
                        log::warn!("Failed to update file {}: {}", file_path.display(), e);
                    }
                }
            }
        }
        
        let duration = start_time.elapsed().unwrap_or_default();
        
        Ok(IndexUpdateResult {
            updated_files,
            added_symbols,
            removed_symbols,
            duration,
        })
    }
    
    /// 単一ファイルの更新
    async fn update_single_file(&self, file_path: &PathBuf) -> Result<(usize, usize)> {
        // 拡張子チェック
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;
            
        if !self.watched_extensions.contains(extension) {
            return Ok((0, 0));
        }
        
        // 現在のシンボル数を取得（削除カウント用）
        let removed_count = {
            let cache_manager = self.cache_manager.lock().unwrap();
            cache_manager.get_file_symbol_count(file_path).unwrap_or(0)
        };
        
        // Tree-sitterによるシンボル抽出
        let new_symbols = tokio::task::spawn_blocking({
            let file_path = file_path.clone();
            move || -> Result<Vec<SymbolMetadata>> {
                extract_symbols_from_file(&file_path)
                    .context("Failed to extract symbols")
            }
        }).await??;
        
        let added_count = new_symbols.len();
        
        // キャッシュ更新
        {
            let mut cache_manager = self.cache_manager.lock().unwrap();
            cache_manager.update_file_symbols(file_path, new_symbols)?;
        }
        
        Ok((added_count, removed_count))
    }
    
    /// ファイルをインデックスから削除
    async fn remove_file_from_index(&self, file_path: &PathBuf) -> Result<()> {
        let mut cache_manager = self.cache_manager.lock().unwrap();
        cache_manager.invalidate_file(file_path);
        log::debug!("Removed file from index: {}", file_path.display());
        Ok(())
    }
    
    /// 監視設定を更新
    pub fn set_debounce_time(&mut self, debounce_ms: u64) {
        self.debounce_ms = debounce_ms;
    }
    
    /// 監視対象拡張子を追加
    pub fn add_watched_extension(&mut self, extension: String) {
        self.watched_extensions.insert(extension);
    }
}

