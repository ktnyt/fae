//! Search debounce control
//!
//! Provides debounced search execution to avoid excessive search requests
//! while the user is typing. Implements a simple delay-based debouncing
//! mechanism with configurable delay duration.

use std::time::{Duration, Instant};

/// Search debounce controller for rate-limiting search requests
#[derive(Debug)]
pub struct SearchDebouncer {
    /// Debounce delay duration (default: 100ms)
    pub debounce_delay: Duration,

    /// Pending search query to execute after debounce delay
    pub pending_search_query: Option<String>,

    /// Last input time for debounce calculation
    pub last_input_time: Option<Instant>,
}

impl SearchDebouncer {
    /// Create new search debouncer with default delay (100ms)
    pub fn new() -> Self {
        Self {
            debounce_delay: Duration::from_millis(100),
            pending_search_query: None,
            last_input_time: None,
        }
    }

    /// Create new search debouncer with custom delay
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            debounce_delay: delay,
            pending_search_query: None,
            last_input_time: None,
        }
    }

    /// Set pending search query and update input time
    pub fn set_pending_search(&mut self, query: String) {
        self.pending_search_query = Some(query);
        self.last_input_time = Some(Instant::now());
    }

    /// Clear pending search
    pub fn clear_pending_search(&mut self) {
        self.pending_search_query = None;
        self.last_input_time = None;
    }

    /// Check if debounce delay has elapsed and return pending query if ready
    pub fn check_ready_for_search(&mut self) -> Option<String> {
        if let (Some(last_time), Some(query)) = (self.last_input_time, &self.pending_search_query) {
            if last_time.elapsed() >= self.debounce_delay {
                let ready_query = query.clone();
                self.clear_pending_search();
                return Some(ready_query);
            }
        }
        None
    }

    /// Check if there's a pending search that hasn't timed out yet
    pub fn has_pending_search(&self) -> bool {
        self.pending_search_query.is_some() && self.last_input_time.is_some()
    }

    /// Get remaining time until debounce delay expires
    pub fn time_until_ready(&self) -> Option<Duration> {
        if let Some(last_time) = self.last_input_time {
            let elapsed = last_time.elapsed();
            if elapsed < self.debounce_delay {
                Some(self.debounce_delay - elapsed)
            } else {
                Some(Duration::ZERO)
            }
        } else {
            None
        }
    }
}

impl Default for SearchDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_search_debouncer_creation() {
        let debouncer = SearchDebouncer::new();
        assert_eq!(debouncer.debounce_delay, Duration::from_millis(100));
        assert!(debouncer.pending_search_query.is_none());
        assert!(debouncer.last_input_time.is_none());
    }

    #[test]
    fn test_search_debouncer_with_custom_delay() {
        let delay = Duration::from_millis(200);
        let debouncer = SearchDebouncer::with_delay(delay);
        assert_eq!(debouncer.debounce_delay, delay);
    }

    #[test]
    fn test_set_pending_search() {
        let mut debouncer = SearchDebouncer::new();
        debouncer.set_pending_search("test query".to_string());

        assert_eq!(
            debouncer.pending_search_query,
            Some("test query".to_string())
        );
        assert!(debouncer.last_input_time.is_some());
        assert!(debouncer.has_pending_search());
    }

    #[test]
    fn test_clear_pending_search() {
        let mut debouncer = SearchDebouncer::new();
        debouncer.set_pending_search("test".to_string());
        debouncer.clear_pending_search();

        assert!(debouncer.pending_search_query.is_none());
        assert!(debouncer.last_input_time.is_none());
        assert!(!debouncer.has_pending_search());
    }

    #[test]
    fn test_check_ready_for_search_immediate() {
        let mut debouncer = SearchDebouncer::new();

        // No pending search - should return None
        assert!(debouncer.check_ready_for_search().is_none());
    }

    #[test]
    fn test_check_ready_for_search_with_delay() {
        let mut debouncer = SearchDebouncer::with_delay(Duration::from_millis(1));
        debouncer.set_pending_search("test".to_string());

        // Wait for delay to pass
        thread::sleep(Duration::from_millis(5));

        // Should be ready now
        let result = debouncer.check_ready_for_search();
        assert_eq!(result, Some("test".to_string()));

        // Should be cleared after retrieval
        assert!(!debouncer.has_pending_search());
    }

    #[test]
    fn test_time_until_ready() {
        let mut debouncer = SearchDebouncer::with_delay(Duration::from_millis(100));

        // No pending search
        assert!(debouncer.time_until_ready().is_none());

        // Set pending search
        debouncer.set_pending_search("test".to_string());
        let remaining = debouncer.time_until_ready();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= Duration::from_millis(100));
    }
}
