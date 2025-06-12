use anyhow::{Context, Result};
use ignore::{WalkBuilder, DirEntry};
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashSet;

/// ファイル発見・監視・Git統合を担当するマネージャー
#[derive(Clone)]
pub struct IndexManager {
    /// プロジェクトルートディレクトリ
    project_root: PathBuf,
    /// 対象とするファイル拡張子
    target_extensions: HashSet<String>,
    /// ファイルサイズ制限（バイト）
    max_file_size: u64,
}

/// ファイル情報
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// ファイルパス
    pub path: PathBuf,
    /// ファイルサイズ（バイト）
    pub size: u64,
    /// 相対パス
    pub relative_path: PathBuf,
}

impl IndexManager {
    /// 新しいIndexManagerを作成
    pub fn new(project_root: PathBuf) -> Self {
        let mut target_extensions = HashSet::new();
        target_extensions.insert("ts".to_string());
        target_extensions.insert("tsx".to_string());
        target_extensions.insert("js".to_string());
        target_extensions.insert("jsx".to_string());
        target_extensions.insert("py".to_string());
        target_extensions.insert("rs".to_string());

        Self {
            project_root,
            target_extensions,
            max_file_size: 1024 * 1024, // 1MB
        }
    }

    /// プロジェクト内の対象ファイルを発見
    pub fn discover_files(&self) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        
        let walker = WalkBuilder::new(&self.project_root)
            .standard_filters(true)  // .gitignore, .ignore, hidden filesを自動処理
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if let Some(file_info) = self.process_entry(entry)? {
                        files.push(file_info);
                    }
                }
                Err(err) => {
                    // ファイルアクセスエラーは警告として記録
                    eprintln!("Warning: Failed to access file: {}", err);
                }
            }
        }

        Ok(files)
    }

    /// Git変更ファイルを取得
    pub fn get_git_changed_files(&self) -> Result<Vec<FileInfo>> {
        // 基本的なファイル発見を行い、後でGit統合を追加
        // 現在はすべてのファイルを返す
        self.discover_files()
    }

    /// 最近変更されたファイルを取得
    pub fn get_recently_modified_files(&self, limit: usize) -> Result<Vec<FileInfo>> {
        let mut files = self.discover_files()?;
        
        // ファイルの最終更新時刻でソート
        files.sort_by(|a, b| {
            let a_modified = fs::metadata(&a.path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let b_modified = fs::metadata(&b.path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            b_modified.cmp(&a_modified) // 新しい順
        });

        files.truncate(limit);
        Ok(files)
    }

    /// プロジェクトルートを取得
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// 対象拡張子を追加
    pub fn add_extension(&mut self, ext: &str) {
        self.target_extensions.insert(ext.to_string());
    }

    /// ファイルサイズ制限を設定
    pub fn set_max_file_size(&mut self, size: u64) {
        self.max_file_size = size;
    }

    /// DirEntryを処理してFileInfoに変換
    fn process_entry(&self, entry: DirEntry) -> Result<Option<FileInfo>> {
        let path = entry.path();
        
        // ディレクトリはスキップ
        if !path.is_file() {
            return Ok(None);
        }

        // 拡張子チェック
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if !self.target_extensions.contains(extension) {
                return Ok(None);
            }
        } else {
            return Ok(None); // 拡張子なしのファイルはスキップ
        }

        // ファイルサイズチェック
        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;
        
        let size = metadata.len();
        if size > self.max_file_size {
            return Ok(None);
        }

        // バイナリファイルチェック（簡易版）
        if self.is_likely_binary(path)? {
            return Ok(None);
        }

        // 相対パスを計算
        let relative_path = path.strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_path_buf();

        Ok(Some(FileInfo {
            path: path.to_path_buf(),
            size,
            relative_path,
        }))
    }

    /// ファイルがバイナリかどうかの簡易判定
    fn is_likely_binary(&self, path: &Path) -> Result<bool> {
        // ファイルの最初の1024バイトを読んで判定
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return Ok(true), // 読めないファイルはバイナリとして扱う
        };

        use std::io::Read;
        let mut buffer = [0; 1024];
        let bytes_read = file.read(&mut buffer)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // NULL文字が含まれていればバイナリとして判定
        for &byte in &buffer[..bytes_read] {
            if byte == 0 {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // テスト用ファイル作成
        let src_dir = root.join("src");
        fs::create_dir(&src_dir)?;

        // TypeScriptファイル
        let mut ts_file = File::create(src_dir.join("main.ts"))?;
        writeln!(ts_file, "function hello() {{ console.log('Hello'); }}")?;

        // Pythonファイル
        let mut py_file = File::create(src_dir.join("script.py"))?;
        writeln!(py_file, "def main():\n    print('Hello')")?;

        // 大きなファイル（除外されるべき）
        let mut large_file = File::create(src_dir.join("large.txt"))?;
        let large_content = "x".repeat(2 * 1024 * 1024); // 2MB
        write!(large_file, "{}", large_content)?;

        // .gitignoreファイル
        let mut gitignore = File::create(root.join(".gitignore"))?;
        writeln!(gitignore, "*.tmp\nignored/")?;

        // 無視されるディレクトリ
        let ignored_dir = root.join("ignored");
        fs::create_dir(&ignored_dir)?;
        let mut ignored_file = File::create(ignored_dir.join("ignored.ts"))?;
        writeln!(ignored_file, "// This should be ignored")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_discover_files() -> Result<()> {
        let temp_dir = create_test_project()?;
        let manager = IndexManager::new(temp_dir.path().to_path_buf());

        let files = manager.discover_files()?;

        // main.ts と script.py が見つかるはず
        assert!(files.len() >= 2);
        
        let file_names: Vec<String> = files.iter()
            .map(|f| f.relative_path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(file_names.contains(&"main.ts".to_string()));
        assert!(file_names.contains(&"script.py".to_string()));
        
        // large.txtは除外されているはず（サイズ制限）
        assert!(!file_names.contains(&"large.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_extension_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path().to_path_buf());

        // 対象拡張子の確認
        assert!(manager.target_extensions.contains("ts"));
        assert!(manager.target_extensions.contains("py"));
        assert!(manager.target_extensions.contains("rs"));
        assert!(!manager.target_extensions.contains("txt"));
    }

    #[test]
    fn test_recently_modified_files() -> Result<()> {
        let temp_dir = create_test_project()?;
        let manager = IndexManager::new(temp_dir.path().to_path_buf());

        let files = manager.get_recently_modified_files(5)?;

        // ファイルが見つかること
        assert!(!files.is_empty());
        
        // 5件以下であること
        assert!(files.len() <= 5);

        Ok(())
    }

    #[test]
    fn test_binary_detection() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = IndexManager::new(temp_dir.path().to_path_buf());

        // テキストファイル
        let text_file = temp_dir.path().join("text.txt");
        let mut file = File::create(&text_file)?;
        writeln!(file, "This is a text file")?;
        assert!(!manager.is_likely_binary(&text_file)?);

        // バイナリファイル（NULL文字含む）
        let binary_file = temp_dir.path().join("binary.bin");
        let mut file = File::create(&binary_file)?;
        file.write_all(&[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00, 0x57, 0x6f, 0x72, 0x6c, 0x64])?;
        assert!(manager.is_likely_binary(&binary_file)?);

        Ok(())
    }
}