use which::which;

/// Represents the available content search backends
#[derive(Debug, Clone, PartialEq)]
pub enum ContentSearchBackend {
    Ripgrep,
    Ag,
    Fallback,
}

impl ContentSearchBackend {
    /// Detect the best available content search backend
    pub fn detect() -> Self {
        // 最適な順序: ripgrep → ag → fallback
        // 実際のベンチマーク結果: fallback(2.74ms) > ripgrep(13.25ms) > ag
        // しかし外部ツールが利用可能な場合は一貫性のため優先使用
        if which("rg").is_ok() {
            ContentSearchBackend::Ripgrep
        } else if which("ag").is_ok() {
            ContentSearchBackend::Ag
        } else {
            ContentSearchBackend::Fallback
        }
    }

    /// Get a human-readable name for the backend
    pub fn name(&self) -> &'static str {
        match self {
            ContentSearchBackend::Ripgrep => "ripgrep",
            ContentSearchBackend::Ag => "ag",
            ContentSearchBackend::Fallback => "fallback",
        }
    }
}