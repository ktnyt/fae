/// Mode detection logic separated from search execution
pub struct ModeDetector;

impl ModeDetector {
    /// Detect which search mode should handle the given query
    pub fn detect_mode_type(query: &str) -> ModeType {
        if query.starts_with('#') {
            ModeType::Symbol
        } else if query.starts_with('>') {
            ModeType::File
        } else if query.starts_with('/') {
            ModeType::Regex
        } else {
            ModeType::Content
        }
    }

    /// Clean query by removing the mode prefix
    pub fn clean_query(query: &str, mode_type: &ModeType) -> String {
        match mode_type {
            ModeType::Symbol => query.strip_prefix('#').unwrap_or(query).to_string(),
            ModeType::File => query.strip_prefix('>').unwrap_or(query).to_string(),
            ModeType::Regex => query.strip_prefix('/').unwrap_or(query).to_string(),
            ModeType::Content => query.to_string(),
        }
    }
}

/// Enumeration of available search mode types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeType {
    Content,
    Symbol,
    File,
    Regex,
}

impl ModeType {
    /// Get the prefix character for this mode type
    pub fn prefix(&self) -> &'static str {
        match self {
            ModeType::Content => "",
            ModeType::Symbol => "#",
            ModeType::File => ">",
            ModeType::Regex => "/",
        }
    }

    /// Get the display name for this mode type
    pub fn name(&self) -> &'static str {
        match self {
            ModeType::Content => "Content",
            ModeType::Symbol => "Symbol", 
            ModeType::File => "File",
            ModeType::Regex => "Regex",
        }
    }

    /// Get the icon for this mode type
    pub fn icon(&self) -> &'static str {
        match self {
            ModeType::Content => "🔍",
            ModeType::Symbol => "🏷️",
            ModeType::File => "📁",
            ModeType::Regex => "🔧",
        }
    }

    /// Get the description for this mode type
    pub fn description(&self) -> &'static str {
        match self {
            ModeType::Content => "Search within file contents using literal search",
            ModeType::Symbol => "Search for code symbols (functions, classes, variables) using fuzzy matching",
            ModeType::File => "Search for files and directories using fuzzy matching",
            ModeType::Regex => "Search within file contents using regular expressions",
        }
    }
}