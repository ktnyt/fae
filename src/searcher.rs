use crate::types::*;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

pub struct FuzzySearcher {
    symbols: Vec<CodeSymbol>,
    matcher: SkimMatcherV2,
}

impl std::fmt::Debug for FuzzySearcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzySearcher")
            .field("symbols", &self.symbols)
            .field("matcher", &"SkimMatcherV2")
            .finish()
    }
}

impl FuzzySearcher {
    pub fn new(symbols: Vec<CodeSymbol>) -> Self {
        Self {
            symbols,
            matcher: SkimMatcherV2::default(),
        }
    }

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
                if !include_files && symbol.symbol_type == SymbolType::Filename {
                    continue;
                }
            }

            if let Some(include_dirs) = options.include_dirs {
                if !include_dirs && symbol.symbol_type == SymbolType::Dirname {
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
        results.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        results
    }

    pub fn update_symbols(&mut self, symbols: Vec<CodeSymbol>) {
        self.symbols = symbols;
    }
}