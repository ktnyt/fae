use super::traits::ExternalSearchBackend;
use crate::types::{SearchResult, DisplayInfo};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// the_silver_searcher (ag) バックエンド
pub struct AgBackend;

impl AgBackend {
    pub fn new() -> Self {
        Self
    }
    
    /// agコマンドの出力を解析してSearchResultに変換
    fn parse_ag_output(&self, output: &str, project_root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        
        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            // ag --vimgrep --column形式: file:line:column:content
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() < 4 {
                continue;
            }
            
            let file_path = project_root.join(parts[0]);
            let line_number: u32 = parts[1].parse().unwrap_or(1);
            let column_number: u32 = parts[2].parse().unwrap_or(1);
            let line_content = parts[3].to_string();
            
            // マッチ位置を正確に計算（大文字小文字を無視して検索）
            let (match_start, match_end) = self.find_match_positions(&line_content, query, column_number);
            
            // 簡易的なスコア計算（agは既に関連度順でソートされている想定）
            let score = 1.0;
            
            let result = SearchResult {
                file_path,
                line: line_number,
                column: column_number,
                display_info: DisplayInfo::Content {
                    line_content: line_content.clone(),
                    match_start,
                    match_end,
                },
                score,
            };
            
            results.push(result);
        }
        
        Ok(results)
    }
    
    /// 行内でのマッチ開始・終了位置を計算（UTF-8安全）
    fn find_match_positions(&self, line_content: &str, query: &str, column_hint: u32) -> (usize, usize) {
        // agのカラムヒントは1-basedなので0-basedに変換
        let column_hint_char_index = (column_hint.saturating_sub(1)) as usize;
        
        // 大文字小文字を無視してマッチ位置を検索
        let line_lower = line_content.to_lowercase();
        let query_lower = query.to_lowercase();
        
        // カラムヒントの位置から検索を開始して正確なマッチを見つける
        let start_char_index = if column_hint_char_index < line_content.chars().count() {
            // カラムヒント位置からのマッチを確認
            let remaining_chars: String = line_content.chars().skip(column_hint_char_index).collect();
            let remaining_lower = remaining_chars.to_lowercase();
            
            if remaining_lower.starts_with(&query_lower) {
                // カラムヒント位置が正確なマッチ開始位置
                column_hint_char_index
            } else {
                // フォールバック：最初のマッチを使用
                line_lower.find(&query_lower)
                    .map(|byte_pos| line_content[..byte_pos].chars().count())
                    .unwrap_or(0)
            }
        } else {
            // カラムヒントが無効な場合：最初のマッチを使用
            line_lower.find(&query_lower)
                .map(|byte_pos| line_content[..byte_pos].chars().count())
                .unwrap_or(0)
        };
        
        let query_char_len = query_lower.chars().count();
        let end_char_index = start_char_index + query_char_len;
        
        // 文字位置をバイト位置に変換
        let start_byte_pos = line_content.char_indices()
            .nth(start_char_index)
            .map(|(i, _)| i)
            .unwrap_or(0);
            
        let end_byte_pos = line_content.char_indices()
            .nth(end_char_index)
            .map(|(i, _)| i)
            .unwrap_or(line_content.len());
            
        (start_byte_pos, end_byte_pos)
    }
}

impl ExternalSearchBackend for AgBackend {
    fn name(&self) -> &'static str {
        "ag"
    }
    
    fn is_available(&self) -> bool {
        Command::new("ag")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    fn search_content(&self, project_root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        let output = Command::new("ag")
            .args([
                "--vimgrep",           // file:line:column:content 形式
                "--column",            // カラム番号を出力
                "--literal",           // リテラル検索（正規表現として解釈しない）
                "-i",                  // 大文字小文字を無視
                query,
            ])
            .current_dir(project_root)
            .output()
            .with_context(|| format!("Failed to execute ag for query: {}", query))?;
        
        if !output.status.success() {
            // agは結果が見つからない場合にexit code 1を返すので、
            // それは正常な動作として扱う
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }
            
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ag failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_ag_output(&stdout, project_root, query)
    }
    
    fn search_regex(&self, project_root: &Path, pattern: &str) -> Result<Vec<SearchResult>> {
        let output = Command::new("ag")
            .args([
                "--vimgrep",           // file:line:column:content 形式
                "--column",            // カラム番号を出力
                "--ignore-case",       // 大文字小文字を無視
                "--skip-vcs-ignores",  // VCSの無視ファイルを読む
                pattern,
            ])
            .current_dir(project_root)
            .output()
            .with_context(|| format!("Failed to execute ag regex search for pattern: {}", pattern))?;
        
        if !output.status.success() {
            // agは結果が見つからない場合にexit code 1を返すので、
            // それは正常な動作として扱う
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }
            
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ag regex search failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        // 正規表現の場合はパターンをそのまま使用
        self.parse_ag_output(&stdout, project_root, pattern)
    }
    
    fn priority(&self) -> u32 {
        90 // rgの次に高い優先度
    }
}