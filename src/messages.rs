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
    UpdateQuery { query: String },
    /// Push a search result to listeners
    PushSearchResult { result: SearchResult },
    /// Change search mode
    SetSearchMode { mode: SearchMode },
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

    /// Convenience constructor for update query
    pub fn update_query(query: String) -> Self {
        FaeMessage::Search(SearchMessage::UpdateQuery { query })
    }

    /// Convenience constructor for push search result
    pub fn push_search_result(result: SearchResult) -> Self {
        FaeMessage::Search(SearchMessage::PushSearchResult { result })
    }

    /// Convenience constructor for set search mode
    pub fn set_search_mode(mode: SearchMode) -> Self {
        FaeMessage::Search(SearchMessage::SetSearchMode { mode })
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
        let search_msg = FaeMessage::update_query("test query".to_string());
        assert!(search_msg.is_search());
        assert!(!search_msg.is_system());
        
        if let Some(SearchMessage::UpdateQuery { query }) = search_msg.as_search() {
            assert_eq!(query, "test query");
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
    fn test_search_mode_message() {
        let msg = FaeMessage::set_search_mode(SearchMode::Literal);
        assert!(msg.is_search());
        
        if let Some(SearchMessage::SetSearchMode { mode }) = msg.as_search() {
            assert_eq!(*mode, SearchMode::Literal);
        } else {
            panic!("Expected SetSearchMode message");
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
        let msg = FaeMessage::update_query("serialization test".to_string());
        
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
}