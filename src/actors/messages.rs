//! Unified message system for fae actors
//!
//! This module defines the FaeMessage enum that encompasses all message types
//! used across different actors in the fae system. This enables broadcast
//! communication between actors of different types.

use serde::{Deserialize, Serialize};

/// Search result data structure for ripgrep output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub filename: String,
    pub line: u32,
    pub offset: u32,
    pub content: String,
}

/// Search mode for ripgrep execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchMode {
    /// Literal string search (exact match)
    Literal,
    /// Regular expression search
    Regexp,
}

/// Search query containing both query string and search mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
}

impl SearchParams {
    /// Create a new search query
    pub fn new(query: String, mode: SearchMode) -> Self {
        Self { query, mode }
    }

    /// Create a literal search query
    pub fn literal(query: String) -> Self {
        Self::new(query, SearchMode::Literal)
    }

    /// Create a regex search query
    pub fn regex(query: String) -> Self {
        Self::new(query, SearchMode::Regexp)
    }
}

/// Unified message enum for all fae actors
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FaeMessage {
    /// Search-related messages
    Search(SearchMessage),

    /// System-related messages (for future expansion)
    System(SystemMessage),
}

/// Search-related message types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchMessage {
    /// Update the current search query
    UpdateQuery { search_params: SearchParams },
    /// Push a search result to listeners
    PushSearchResult { result: SearchResult },
    /// Clear all search results
    ClearResults,
}

/// System-related message types (for future expansion)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SystemMessage {
    /// Shutdown signal
    Shutdown,
    /// Status query
    GetStatus,
    /// Configuration update
    UpdateConfig { key: String, value: String },
}

impl FaeMessage {
    /// Convenience constructor for search messages
    pub fn search(message: SearchMessage) -> Self {
        FaeMessage::Search(message)
    }

    /// Convenience constructor for system messages
    pub fn system(message: SystemMessage) -> Self {
        FaeMessage::System(message)
    }

    /// Convenience constructor for update query with mode
    pub fn update_query(query: String, mode: SearchMode) -> Self {
        FaeMessage::Search(SearchMessage::UpdateQuery {
            search_params: SearchParams::new(query, mode),
        })
    }

    /// Convenience constructor for update query with SearchQuery
    pub fn update_search_query(search_params: SearchParams) -> Self {
        FaeMessage::Search(SearchMessage::UpdateQuery { search_params })
    }

    /// Convenience constructor for push search result
    pub fn push_search_result(result: SearchResult) -> Self {
        FaeMessage::Search(SearchMessage::PushSearchResult { result })
    }

    /// Convenience constructor for clear results
    pub fn clear_results() -> Self {
        FaeMessage::Search(SearchMessage::ClearResults)
    }

    /// Check if this is a search message
    pub fn is_search(&self) -> bool {
        matches!(self, FaeMessage::Search(_))
    }

    /// Check if this is a system message
    pub fn is_system(&self) -> bool {
        matches!(self, FaeMessage::System(_))
    }

    /// Extract search message if this is a search message
    pub fn as_search(&self) -> Option<&SearchMessage> {
        match self {
            FaeMessage::Search(msg) => Some(msg),
            _ => None,
        }
    }

    /// Extract system message if this is a system message
    pub fn as_system(&self) -> Option<&SystemMessage> {
        match self {
            FaeMessage::System(msg) => Some(msg),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fae_message_construction() {
        // Test search message construction
        let search_msg = FaeMessage::update_query("test query".to_string(), SearchMode::Literal);
        assert!(search_msg.is_search());
        assert!(!search_msg.is_system());

        if let Some(SearchMessage::UpdateQuery { search_params }) = search_msg.as_search() {
            assert_eq!(search_params.query, "test query");
            assert_eq!(search_params.mode, SearchMode::Literal);
        } else {
            panic!("Expected UpdateQuery message");
        }
    }

    #[test]
    fn test_search_result_message() {
        let result = SearchResult {
            filename: "test.rs".to_string(),
            line: 42,
            offset: 10,
            content: "fn test() {}".to_string(),
        };

        let msg = FaeMessage::push_search_result(result.clone());
        assert!(msg.is_search());

        if let Some(SearchMessage::PushSearchResult { result: r }) = msg.as_search() {
            assert_eq!(r.filename, result.filename);
            assert_eq!(r.line, result.line);
            assert_eq!(r.offset, result.offset);
            assert_eq!(r.content, result.content);
        } else {
            panic!("Expected PushSearchResult message");
        }
    }

    #[test]
    fn test_search_mode_in_query() {
        let msg = FaeMessage::update_query("pattern".to_string(), SearchMode::Regexp);
        assert!(msg.is_search());

        if let Some(SearchMessage::UpdateQuery { search_params }) = msg.as_search() {
            assert_eq!(search_params.query, "pattern");
            assert_eq!(search_params.mode, SearchMode::Regexp);
        } else {
            panic!("Expected UpdateQuery message with mode");
        }
    }

    #[test]
    fn test_system_message() {
        let msg = FaeMessage::system(SystemMessage::Shutdown);
        assert!(msg.is_system());
        assert!(!msg.is_search());

        if let Some(SystemMessage::Shutdown) = msg.as_system() {
            // Test passed
        } else {
            panic!("Expected Shutdown message");
        }
    }

    #[test]
    fn test_message_serialization() {
        let msg = FaeMessage::update_query("serialization test".to_string(), SearchMode::Literal);

        // Test that messages can be serialized and deserialized
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: FaeMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_clear_results_message() {
        let msg = FaeMessage::clear_results();
        assert!(msg.is_search());

        if let Some(SearchMessage::ClearResults) = msg.as_search() {
            // Test passed
        } else {
            panic!("Expected ClearResults message");
        }
    }

    #[test]
    fn test_search_query_construction() {
        // Test direct construction
        let query = SearchParams::new("test".to_string(), SearchMode::Literal);
        assert_eq!(query.query, "test");
        assert_eq!(query.mode, SearchMode::Literal);

        // Test convenience constructors
        let literal_query = SearchParams::literal("literal test".to_string());
        assert_eq!(literal_query.query, "literal test");
        assert_eq!(literal_query.mode, SearchMode::Literal);

        let regex_query = SearchParams::regex("regex.*test".to_string());
        assert_eq!(regex_query.query, "regex.*test");
        assert_eq!(regex_query.mode, SearchMode::Regexp);
    }

    #[test]
    fn test_search_query_serialization() {
        let query = SearchParams::new("serialization test".to_string(), SearchMode::Regexp);

        // Test that SearchQuery can be serialized and deserialized
        let serialized = serde_json::to_string(&query).unwrap();
        let deserialized: SearchParams = serde_json::from_str(&serialized).unwrap();

        assert_eq!(query, deserialized);
    }

    #[test]
    fn test_update_search_query_message() {
        let search_params = SearchParams::regex("pattern.*test".to_string());
        let msg = FaeMessage::update_search_query(search_params.clone());

        assert!(msg.is_search());
        if let Some(SearchMessage::UpdateQuery { search_params: sq }) = msg.as_search() {
            assert_eq!(sq.query, "pattern.*test");
            assert_eq!(sq.mode, SearchMode::Regexp);
        } else {
            panic!("Expected UpdateQuery message with SearchQuery");
        }
    }
}
