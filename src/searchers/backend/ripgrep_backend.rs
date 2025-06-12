use super::traits::ExternalSearchBackend;
use crate::types::{SearchResult, DisplayInfo};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// ripgrep (rg) バックエンド
pub struct RipgrepBackend;

impl RipgrepBackend {
    pub fn new() -> Self {
        Self
    }
    
    /// rgコマンドの出力を解析してSearchResultに変換
    fn parse_rg_output(&self, output: &str, project_root: &Path) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        
        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            // rg --vimgrep形式: file:line:column:content
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() < 4 {
                continue;
            }
            
            let file_path = project_root.join(parts[0]);
            let line_number: u32 = parts[1].parse().unwrap_or(1);
            let column_number: u32 = parts[2].parse().unwrap_or(1);
            let line_content = parts[3].to_string();
            
            // 簡易的なスコア計算（ripgrepは既に関連度順でソートされている想定）
            let score = 1.0;
            
            let result = SearchResult {
                file_path,
                line: line_number,
                column: column_number,
                display_info: DisplayInfo::Content {
                    line_content: line_content.clone(),
                    match_start: 0, // rgの出力からは正確な位置が取得困難
                    match_end: 0,
                },
                score,
            };
            
            results.push(result);
        }
        
        Ok(results)
    }
}

impl ExternalSearchBackend for RipgrepBackend {
    fn name(&self) -> &'static str {
        "ripgrep"
    }
    
    fn is_available(&self) -> bool {
        Command::new("rg")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    fn search_content(&self, project_root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        let output = Command::new("rg")
            .args([
                "--vimgrep",           // file:line:column:content 形式
                "-i",                  // 大文字小文字を無視
                "--type-not", "binary", // バイナリファイルを除外
                "--max-filesize", "1M", // 1MB以上のファイルを除外
                query,
            ])
            .current_dir(project_root)
            .output()
            .with_context(|| format!("Failed to execute ripgrep for query: {}", query))?;
        
        if !output.status.success() {
            // rgは結果が見つからない場合にexit code 1を返すので、
            // それは正常な動作として扱う
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }
            
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ripgrep failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_rg_output(&stdout, project_root)
    }
    
    fn priority(&self) -> u32 {
        100 // 最高優先度
    }
}