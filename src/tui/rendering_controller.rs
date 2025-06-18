//! Rendering control and throttling
//!
//! Provides rendering control with throttling to maintain optimal frame rates
//! and avoid unnecessary CPU usage. Implements a simple time-based throttling
//! mechanism with configurable frame rate limits.

use std::time::{Duration, Instant};

/// Rendering controller for throttling UI updates and managing redraw state
#[derive(Debug)]
pub struct RenderingController {
    /// Whether a redraw is needed
    pub needs_redraw: bool,

    /// Last time the UI was drawn
    pub last_draw_time: Instant,

    /// Minimum time between redraws (throttle duration)
    pub draw_throttle_duration: Duration,
}

impl RenderingController {
    /// Create new rendering controller with default settings (60 FPS)
    pub fn new() -> Self {
        Self {
            needs_redraw: true, // Initial draw needed
            last_draw_time: Instant::now(),
            draw_throttle_duration: Duration::from_millis(16), // 60 FPS (~16.67ms)
        }
    }

    /// Create new rendering controller with custom frame rate
    pub fn with_fps(fps: u32) -> Self {
        let frame_duration = Duration::from_millis(1000 / fps as u64);
        Self {
            needs_redraw: true,
            last_draw_time: Instant::now(),
            draw_throttle_duration: frame_duration,
        }
    }

    /// Create new rendering controller with custom throttle duration
    pub fn with_throttle_duration(duration: Duration) -> Self {
        Self {
            needs_redraw: true,
            last_draw_time: Instant::now(),
            draw_throttle_duration: duration,
        }
    }

    /// Request a redraw
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Check if enough time has passed since last draw to allow a new draw
    pub fn should_draw(&self) -> bool {
        if !self.needs_redraw {
            return false;
        }

        let now = Instant::now();
        now.duration_since(self.last_draw_time) >= self.draw_throttle_duration
    }

    /// Mark that a draw operation has been completed
    pub fn mark_drawn(&mut self) {
        self.needs_redraw = false;
        self.last_draw_time = Instant::now();
    }

    /// Get time until next draw is allowed
    pub fn time_until_next_draw(&self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_draw_time);

        if elapsed >= self.draw_throttle_duration {
            Duration::ZERO
        } else {
            self.draw_throttle_duration - elapsed
        }
    }

    /// Check if a redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Force an immediate redraw (bypassing throttle)
    pub fn force_redraw(&mut self) {
        self.needs_redraw = true;
        self.last_draw_time = Instant::now() - self.draw_throttle_duration;
    }

    /// Get current frame rate in FPS
    pub fn current_fps(&self) -> f64 {
        1000.0 / self.draw_throttle_duration.as_millis() as f64
    }
}

impl Default for RenderingController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rendering_controller_creation() {
        let controller = RenderingController::new();
        assert!(controller.needs_redraw);
        assert_eq!(controller.draw_throttle_duration, Duration::from_millis(16));
    }

    #[test]
    fn test_rendering_controller_with_fps() {
        let controller = RenderingController::with_fps(30);
        assert_eq!(controller.draw_throttle_duration, Duration::from_millis(33)); // 1000/30
        assert!(controller.needs_redraw);
    }

    #[test]
    fn test_rendering_controller_with_custom_duration() {
        let duration = Duration::from_millis(50);
        let controller = RenderingController::with_throttle_duration(duration);
        assert_eq!(controller.draw_throttle_duration, duration);
    }

    #[test]
    fn test_request_redraw() {
        let mut controller = RenderingController::new();
        controller.needs_redraw = false;

        controller.request_redraw();
        assert!(controller.needs_redraw);
    }

    #[test]
    fn test_should_draw_immediate() {
        let mut controller = RenderingController::new();
        // Initial state should allow drawing
        assert!(controller.should_draw());

        // After marking drawn, should not draw immediately (unless throttle passed)
        controller.mark_drawn();
        assert!(!controller.should_draw());
    }

    #[test]
    fn test_should_draw_with_throttle() {
        let mut controller = RenderingController::with_throttle_duration(Duration::from_millis(10));

        // Mark drawn and request redraw
        controller.mark_drawn();
        controller.request_redraw();

        // Should not draw immediately due to throttle
        assert!(!controller.should_draw());

        // Wait for throttle to pass
        thread::sleep(Duration::from_millis(15));

        // Should now allow drawing
        assert!(controller.should_draw());
    }

    #[test]
    fn test_mark_drawn() {
        let mut controller = RenderingController::new();
        controller.request_redraw();

        controller.mark_drawn();
        assert!(!controller.needs_redraw);
    }

    #[test]
    fn test_time_until_next_draw() {
        let mut controller =
            RenderingController::with_throttle_duration(Duration::from_millis(100));
        controller.mark_drawn();

        let remaining = controller.time_until_next_draw();
        assert!(remaining <= Duration::from_millis(100));
        assert!(remaining > Duration::ZERO);
    }

    #[test]
    fn test_force_redraw() {
        let mut controller = RenderingController::new();
        controller.mark_drawn();
        controller.request_redraw();

        // Should not draw due to throttle
        assert!(!controller.should_draw());

        // Force redraw should bypass throttle
        controller.force_redraw();
        assert!(controller.should_draw());
    }

    #[test]
    fn test_current_fps() {
        let controller = RenderingController::with_fps(60);
        assert!((controller.current_fps() - 60.0).abs() < 1.0); // Allow for rounding

        let controller = RenderingController::with_fps(30);
        assert!((controller.current_fps() - 30.0).abs() < 1.0);
    }
}
