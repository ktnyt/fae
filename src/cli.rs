//! CLI argument parsing and search mode detection

use crate::actors::types::{SearchMode, SearchParams};

/// Search mode prefixes for consistent use across the application
pub const PREFIX_SYMBOL: char = '#';
pub const PREFIX_VARIABLE: char = '$';
pub const PREFIX_FILEPATH: char = '>';
pub const PREFIX_REGEX: char = '/';

/// Parse CLI query argument and determine search mode based on prefix
///
/// # Arguments
/// * `query` - Raw query string from CLI argument
///
/// # Returns
/// Tuple of (SearchMode, stripped_query)
///
/// # Examples
/// ```
/// use fae::cli::parse_query_with_mode;
/// use fae::actors::types::SearchMode;
///
/// let (mode, query) = parse_query_with_mode("hello");
/// assert_eq!(mode, SearchMode::Literal);
/// assert_eq!(query, "hello");
///
/// let (mode, query) = parse_query_with_mode("#my_function");
/// assert_eq!(mode, SearchMode::Symbol);
/// assert_eq!(query, "my_function");
///
/// let (mode, query) = parse_query_with_mode("$my_variable");
/// assert_eq!(mode, SearchMode::Variable);
/// assert_eq!(query, "my_variable");
///
/// let (mode, query) = parse_query_with_mode("@main.rs");
/// assert_eq!(mode, SearchMode::Filepath);
/// assert_eq!(query, "main.rs");
///
/// let (mode, query) = parse_query_with_mode("/fn \\w+");
/// assert_eq!(mode, SearchMode::Regexp);
/// assert_eq!(query, "fn \\w+");
/// ```
pub fn parse_query_with_mode(query: &str) -> (SearchMode, String) {
    if let Some(stripped) = query.strip_prefix(PREFIX_SYMBOL) {
        (SearchMode::Symbol, stripped.to_string())
    } else if let Some(stripped) = query.strip_prefix(PREFIX_VARIABLE) {
        (SearchMode::Variable, stripped.to_string())
    } else if let Some(stripped) = query.strip_prefix(PREFIX_FILEPATH) {
        (SearchMode::Filepath, stripped.to_string())
    } else if let Some(stripped) = query.strip_prefix(PREFIX_REGEX) {
        (SearchMode::Regexp, stripped.to_string())
    } else {
        (SearchMode::Literal, query.to_string())
    }
}

/// Create SearchParams from CLI query argument
///
/// This is a convenience function that combines query parsing and SearchParams construction
pub fn create_search_params(query: &str) -> SearchParams {
    let (mode, parsed_query) = parse_query_with_mode(query);
    SearchParams {
        query: parsed_query,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_literal_mode() {
        let (mode, query) = parse_query_with_mode("hello world");
        assert_eq!(mode, SearchMode::Literal);
        assert_eq!(query, "hello world");
    }

    #[test]
    fn test_parse_query_symbol_mode() {
        let (mode, query) = parse_query_with_mode("#my_function");
        assert_eq!(mode, SearchMode::Symbol);
        assert_eq!(query, "my_function");

        // Test empty symbol query
        let (mode, query) = parse_query_with_mode("#");
        assert_eq!(mode, SearchMode::Symbol);
        assert_eq!(query, "");
    }

    #[test]
    fn test_parse_query_variable_mode() {
        let (mode, query) = parse_query_with_mode("$my_variable");
        assert_eq!(mode, SearchMode::Variable);
        assert_eq!(query, "my_variable");

        let (mode, query) = parse_query_with_mode("$MY_CONSTANT");
        assert_eq!(mode, SearchMode::Variable);
        assert_eq!(query, "MY_CONSTANT");
    }

    #[test]
    fn test_parse_query_filepath_mode() {
        let (mode, query) = parse_query_with_mode("@main.rs");
        assert_eq!(mode, SearchMode::Filepath);
        assert_eq!(query, "main.rs");

        let (mode, query) = parse_query_with_mode("@src/lib.rs");
        assert_eq!(mode, SearchMode::Filepath);
        assert_eq!(query, "src/lib.rs");
    }

    #[test]
    fn test_parse_query_regexp_mode() {
        let (mode, query) = parse_query_with_mode("/fn \\w+");
        assert_eq!(mode, SearchMode::Regexp);
        assert_eq!(query, "fn \\w+");

        let (mode, query) = parse_query_with_mode("/[A-Z]+_[A-Z]+");
        assert_eq!(mode, SearchMode::Regexp);
        assert_eq!(query, "[A-Z]+_[A-Z]+");
    }

    #[test]
    fn test_prefix_edge_cases() {
        // Multiple prefixes - should only strip first
        let (mode, query) = parse_query_with_mode("##nested_symbol");
        assert_eq!(mode, SearchMode::Symbol);
        assert_eq!(query, "#nested_symbol");

        // Mixed prefixes
        let (mode, query) = parse_query_with_mode("#$mixed");
        assert_eq!(mode, SearchMode::Symbol);
        assert_eq!(query, "$mixed");

        // Prefix-like content but literal
        let (mode, query) = parse_query_with_mode("email@domain.com");
        assert_eq!(mode, SearchMode::Literal);
        assert_eq!(query, "email@domain.com");
    }

    #[test]
    fn test_create_search_params() {
        let params = create_search_params("hello");
        assert_eq!(params.mode, SearchMode::Literal);
        assert_eq!(params.query, "hello");

        let params = create_search_params("#function_name");
        assert_eq!(params.mode, SearchMode::Symbol);
        assert_eq!(params.query, "function_name");

        let params = create_search_params("$variable_name");
        assert_eq!(params.mode, SearchMode::Variable);
        assert_eq!(params.query, "variable_name");
    }
}
