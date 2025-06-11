use crate::types::{CodeSymbol, SearchOptions, SearchResult};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

/// Fuzzy search engine for symbols and file names
pub struct FuzzySearchEngine {
    symbols: Vec<CodeSymbol>,
    matcher: SkimMatcherV2,
}

impl std::fmt::Debug for FuzzySearchEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzySearchEngine")
            .field("symbols", &self.symbols)
            .field("matcher", &"SkimMatcherV2")
            .finish()
    }
}

impl FuzzySearchEngine {
    /// Create new fuzzy search engine
    pub fn new(symbols: Vec<CodeSymbol>) -> Self {
        Self {
            symbols,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Update the symbols for fuzzy search
    pub fn update_symbols(&mut self, symbols: Vec<CodeSymbol>) {
        self.symbols = symbols;
    }

    /// Perform fuzzy search on symbols
    pub fn search(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // Handle empty query
        if query.trim().is_empty() {
            return vec![];
        }

        let mut results: Vec<SearchResult> = Vec::new();

        for symbol in &self.symbols {
            // Apply type filtering
            if let Some(ref types) = options.types {
                if !types.contains(&symbol.symbol_type) {
                    continue;
                }
            }

            // Apply file/directory filtering
            if let Some(include_files) = options.include_files {
                if !include_files && symbol.symbol_type == crate::types::SymbolType::Filename {
                    continue;
                }
            }

            if let Some(include_dirs) = options.include_dirs {
                if !include_dirs && symbol.symbol_type == crate::types::SymbolType::Dirname {
                    continue;
                }
            }

            // Perform fuzzy matching
            if let Some((score, _)) = self.matcher.fuzzy_indices(&symbol.name, query) {
                // Convert skim score to distance (higher skim score = better match = lower distance)
                let mut distance = 1.0 - (score as f64 / 100.0);

                // Boost exact matches
                if symbol.name.eq_ignore_ascii_case(query) {
                    distance *= 0.1; // Much better score for exact matches
                }

                // Apply threshold filtering
                if let Some(threshold) = options.threshold {
                    if distance > threshold {
                        continue;
                    }
                }

                results.push(SearchResult {
                    symbol: symbol.clone(),
                    score: distance,
                });
            }
        }

        // Sort by score (ascending = better match first)
        results.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        results
    }

    /// Get the symbols for inspection
    pub fn symbols(&self) -> &[CodeSymbol] {
        &self.symbols
    }
}