//! Toast notification system
//!
//! Provides a simple toast notification system with automatic timeout handling.
//! Supports different toast types (Info, Success, Warning, Error) with appropriate
//! styling and configurable display duration.

use std::time::{Duration, Instant};

/// Toast notification state and content
#[derive(Clone, Debug)]
pub struct ToastState {
    pub visible: bool,
    pub message: String,
    pub toast_type: ToastType,
    pub show_until: Option<Instant>,
}

/// Type of toast notification
#[derive(Clone, Debug, PartialEq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl Default for ToastState {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastState {
    /// Create a new hidden toast
    pub fn new() -> Self {
        Self {
            visible: false,
            message: String::new(),
            toast_type: ToastType::Info,
            show_until: None,
        }
    }

    /// Show a toast with specified message and type for given duration
    pub fn show(&mut self, message: String, toast_type: ToastType, duration: Duration) {
        self.visible = true;
        self.message = message;
        self.toast_type = toast_type;
        self.show_until = Some(Instant::now() + duration);
    }

    /// Update toast state - hide if expired
    pub fn update(&mut self) {
        if let Some(until) = self.show_until {
            if Instant::now() >= until {
                self.hide();
            }
        }
    }

    /// Hide the toast
    pub fn hide(&mut self) {
        self.visible = false;
        self.show_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_state_creation() {
        let toast = ToastState::new();
        assert!(!toast.visible);
        assert_eq!(toast.message, "");
    }

    #[test]
    fn test_toast_state_basic_operations() {
        let mut toast = ToastState::new();

        // Show message
        toast.show("test".to_string(), ToastType::Info, Duration::from_secs(2));
        assert!(toast.visible);
        assert_eq!(toast.message, "test");
        assert_eq!(toast.toast_type, ToastType::Info);

        // Hide message
        toast.hide();
        assert!(!toast.visible);

        // Update message with different type
        toast.show(
            "different".to_string(),
            ToastType::Success,
            Duration::from_secs(3),
        );
        assert!(toast.visible);
        assert_eq!(toast.message, "different");
        assert_eq!(toast.toast_type, ToastType::Success);
    }

    #[test]
    fn test_toast_timeout() {
        let mut toast = ToastState::new();

        // Show toast with very short duration
        toast.show(
            "timeout test".to_string(),
            ToastType::Warning,
            Duration::from_millis(1),
        );
        assert!(toast.visible);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(10));

        // Update should hide the toast
        toast.update();
        assert!(!toast.visible);
    }

    #[test]
    fn test_toast_types() {
        let mut toast = ToastState::new();

        // Test all toast types
        for toast_type in [
            ToastType::Info,
            ToastType::Success,
            ToastType::Warning,
            ToastType::Error,
        ] {
            toast.show(
                "test".to_string(),
                toast_type.clone(),
                Duration::from_secs(1),
            );
            assert_eq!(toast.toast_type, toast_type);
        }
    }
}
