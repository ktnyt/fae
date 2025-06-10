use std::path::Path;
use ignore::WalkBuilder;
use anyhow::Result;

pub struct GitignoreFilter {
    respect_gitignore: bool,
    verbose: bool,
}

impl GitignoreFilter {
    pub fn new(respect_gitignore: bool, verbose: bool) -> Self {
        Self { 
            respect_gitignore,
            verbose,
        }
    }

    /// Create a WalkBuilder configured for gitignore handling
    pub fn create_walker(&self, directory: &Path) -> WalkBuilder {
        let mut builder = WalkBuilder::new(directory);
        
        if self.respect_gitignore {
            builder.git_ignore(true)       // .gitignore files
                   .git_global(true)       // global .gitignore
                   .git_exclude(true)      // .git/info/exclude
                   .require_git(false)     // don't require git repo
                   .hidden(false)          // show hidden files but respect .gitignore
                   .parents(true)          // respect parent .gitignore files
                   .ignore(true)           // respect .ignore files
                   .add_custom_ignore_filename(".ignore"); // custom ignore files
        } else {
            // When not respecting gitignore, explicitly disable all gitignore features
            builder.git_ignore(false)      // disable .gitignore files
                   .git_global(false)      // disable global .gitignore
                   .git_exclude(false)     // disable .git/info/exclude
                   .require_git(false)     // don't require git repo
                   .hidden(false)          // show hidden files
                   .parents(false)         // don't respect parent .gitignore files
                   .ignore(false)          // don't respect .ignore files
                   .filter_entry(|entry| {
                        let path = entry.path();
                        if let Some(path_str) = path.to_str() {
                            // Skip .git directory and its contents
                            if path_str.contains("/.git/") || path_str.ends_with("/.git") {
                                return false;
                            }
                        }
                        true
                    });
        }
        
        builder
    }

    /// Check if a directory entry should be processed
    pub fn should_process_entry(&self, entry_result: &Result<ignore::DirEntry, ignore::Error>) -> Option<std::path::PathBuf> {
        match entry_result {
            Ok(dir_entry) => {
                let path = dir_entry.path();
                
                // Skip .git directory and other common build/cache directories
                if let Some(path_str) = path.to_str() {
                    if path_str.contains("/.git/") || path_str.ends_with("/.git") {
                        return None;
                    }
                }
                
                if path.is_file() {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            }
            Err(e) => {
                if self.verbose {
                    eprintln!("Warning: Failed to read directory entry: {}", e);
                }
                None
            }
        }
    }
}