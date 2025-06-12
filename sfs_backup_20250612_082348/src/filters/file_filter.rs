use std::path::Path;

pub struct FileFilter {
    verbose: bool,
}

impl FileFilter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Check if a file should be indexed based on various criteria
    pub fn should_index_file(&self, path: &Path) -> bool {
        // Check file size - skip files larger than 1MB by default
        const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                if self.verbose {
                    println!(
                        "Skipping large file: {} ({} bytes)",
                        path.display(),
                        metadata.len()
                    );
                }
                return false;
            }
        }

        // Skip binary files and common non-source files
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if self.is_binary_extension(extension) {
                if self.verbose {
                    println!("Skipping binary/non-source file: {}", path.display());
                }
                return false;
            }
        }

        // Skip files with suspicious names (likely generated/cache)
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            if self.is_suspicious_filename(filename) {
                if self.verbose {
                    println!("Skipping suspicious file: {}", path.display());
                }
                return false;
            }
        }

        true
    }

    /// Check if file extension indicates a binary or non-source file
    fn is_binary_extension(&self, extension: &str) -> bool {
        let binary_extensions = [
            // Images
            "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp", // Archives
            "zip", "tar", "gz", "bz2", "7z", "rar", // Executables/binaries
            "exe", "bin", "so", "dylib", "dll", "app", // Media
            "mp3", "mp4", "avi", "mov", "wmv", "flv", // Documents
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", // Databases
            "db", "sqlite", "sqlite3", // Fonts
            "ttf", "otf", "woff", "woff2",
            // Build artifacts (common ones not in .gitignore)
            "o", "obj", "pyc", "class", "jar", // Lock files (often very large)
            "lock",
        ];

        binary_extensions.contains(&extension.to_lowercase().as_str())
    }

    /// Check if filename suggests a temporary or generated file
    fn is_suspicious_filename(&self, filename: &str) -> bool {
        let suspicious_patterns = [
            // Temporary files
            "~",
            ".tmp",
            ".temp",
            ".bak",
            ".backup",
            // IDE files
            ".idea",
            ".vscode",
            // OS files
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            // Log files
            ".log",
        ];

        for pattern in &suspicious_patterns {
            if filename.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Check if file matches the given glob patterns
    pub fn matches_patterns(&self, path: &Path, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return true;
        }

        for pattern in patterns {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches_path(path) {
                    return true;
                }
            }
        }
        false
    }
}
