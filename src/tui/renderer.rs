//! TUI rendering system
//!
//! Provides centralized rendering functionality for all TUI components.
//! Handles layout management, component rendering, and visual state representation.

use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

use super::{IndexStatus, ToastState, ToastType};
use crate::actors::types::SearchMode;
use crate::cli::parse_query_with_mode;

/// Central renderer for TUI application
pub struct TuiRenderer;

impl TuiRenderer {
    /// Render the complete TUI interface
    pub fn render<B: Backend>(
        terminal: &mut Terminal<B>,
        search_input: &str,
        cursor_position: usize,
        search_results: &[String],
        selected_index: Option<usize>,
        toast_state: &ToastState,
        index_status: &IndexStatus,
        show_stats_overlay: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Input box
                    Constraint::Min(1),    // Results box
                    Constraint::Length(3), // Status bar
                ])
                .split(f.size());

            // 1. Input box
            Self::render_input_box(f, chunks[0], search_input, cursor_position);

            // 2. Results box
            Self::render_results_box(f, chunks[1], search_results, selected_index);

            // 3. Status bar
            Self::render_status_bar(f, chunks[2], index_status);

            // 4. Toast (if visible)
            if toast_state.visible {
                Self::render_toast(f, toast_state);
            }

            // 5. Statistics overlay (if visible)
            if show_stats_overlay {
                Self::render_stats_overlay(f, index_status);
            }
        })?;
        Ok(())
    }

    /// Render the input box with search mode indicator and cursor
    fn render_input_box(f: &mut Frame, area: Rect, search_input: &str, cursor_pos: usize) {
        // Detect current search mode
        let (mode, _) = parse_query_with_mode(search_input);
        let mode_name = match mode {
            SearchMode::Literal => "Text",
            SearchMode::Symbol => "Symbol (#)",
            SearchMode::Variable => "Variable ($)",
            SearchMode::Filepath => "File (@)",
            SearchMode::Regexp => "Regex (/)",
        };

        let title = format!("Search Input - {} Mode", mode_name);

        // Create input string with visible cursor
        let display_text = if search_input.is_empty() {
            if cursor_pos == 0 {
                "█".to_string() // Block cursor at position 0
            } else {
                search_input.to_string()
            }
        } else {
            let mut chars: Vec<char> = search_input.chars().collect();
            match cursor_pos.cmp(&chars.len()) {
                std::cmp::Ordering::Less => {
                    // Insert cursor before the character at cursor_pos
                    chars.insert(cursor_pos, '█');
                }
                std::cmp::Ordering::Equal => {
                    // Cursor at end of string
                    chars.push('█');
                }
                std::cmp::Ordering::Greater => {
                    // Cursor beyond string (shouldn't happen, but handle gracefully)
                }
            }
            chars.into_iter().collect()
        };

        let input = Paragraph::new(display_text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White));
        f.render_widget(input, area);
    }

    /// Render the results box with cursor highlighting and automatic scrolling
    fn render_results_box(
        f: &mut Frame,
        area: Rect,
        search_results: &[String],
        selected_index: Option<usize>,
    ) {
        let items: Vec<ListItem> = search_results
            .iter()
            .map(|result| ListItem::new(result.as_str()))
            .collect();

        let title = if let Some(index) = selected_index {
            format!("Search Results ({}/{})", index + 1, search_results.len())
        } else {
            "Search Results".to_string()
        };

        let results_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

        // Create a temporary ListState for rendering with current selection
        let mut list_state = ListState::default();
        list_state.select(selected_index);

        f.render_stateful_widget(results_list, area, &mut list_state);
    }

    /// Render the status bar with help text
    fn render_status_bar(f: &mut Frame, area: Rect, _index_status: &IndexStatus) {
        // Updated help text to include emacs-style bindings
        let help_text = "↑↓/C-p/C-n: Navigate | Enter: Select | Tab: Cycle modes | C-a/e: Start/End | C-k/y: Kill/Yank | C-g: Abort | Esc: Quit";
        let help_status = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(help_status, area);
    }

    /// Render toast notification
    fn render_toast(f: &mut Frame, toast_state: &ToastState) {
        // Calculate optimal size in absolute dimensions
        let (width_chars, height_lines) =
            Self::calculate_toast_size_absolute(toast_state, f.size());

        // Create a top-right positioned popup area with exact dimensions
        let popup_area = Self::top_right_rect_absolute(width_chars, height_lines, f.size());

        // Clear the area first
        f.render_widget(Clear, popup_area);

        // Choose color and title based on toast type
        let (border_color, text_color, title) = match toast_state.toast_type {
            ToastType::Info => (Color::Blue, Color::White, "🔔 Info"),
            ToastType::Success => (Color::Green, Color::White, "✅ Success"),
            ToastType::Warning => (Color::Yellow, Color::Black, "⚠️ Warning"),
            ToastType::Error => (Color::Red, Color::White, "❌ Error"),
        };

        // Get display message
        let display_message = Self::get_toast_display_message(toast_state);

        let toast = Paragraph::new(display_message)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color)),
            )
            .style(Style::default().fg(text_color))
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(toast, popup_area);
    }

    /// Render statistics overlay
    fn render_stats_overlay(f: &mut Frame, index_status: &IndexStatus) {
        // Create a centered popup area
        let area = Self::centered_rect(50, 30, f.size());

        // Clear the area first
        f.render_widget(Clear, area);

        // Create statistics content
        let stats_text = format!(
            "Statistics:\n\nIndexed Files: {}\nQueued Files: {}\nSymbols Found: {}\nStatus: {}",
            index_status.indexed_files,
            index_status.queued_files,
            index_status.symbols_found,
            if index_status.is_active {
                "Active"
            } else {
                "Ready"
            }
        );

        let stats = Paragraph::new(stats_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("📊 Statistics")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Left);

        f.render_widget(stats, area);
    }

    /// Calculate toast size in absolute character dimensions
    pub fn calculate_toast_size_absolute(toast_state: &ToastState, screen_size: Rect) -> (u16, u16) {
        let display_message = Self::get_toast_display_message(toast_state);
        let max_width = screen_size.width.saturating_sub(4); // Leave margins
        let min_width = 20; // Minimum readable width

        // Calculate title width for different toast types
        let title_width: u16 = match toast_state.toast_type {
            crate::tui::ToastType::Info => 9,      // "🔔 Info" = 9 chars
            crate::tui::ToastType::Success => 11,  // "✅ Success" = 11 chars  
            crate::tui::ToastType::Warning => 11,  // "⚠️ Warning" = 11 chars
            crate::tui::ToastType::Error => 9,     // "❌ Error" = 9 chars
        };

        // Calculate optimal width based on content and constraints
        // Add extra space for borders and padding: +4 (2 for borders, 2 for padding)
        let content_width = display_message.chars().count() as u16;
        let required_width_content = content_width.saturating_add(4); // Account for borders and padding
        let required_width_title = title_width.saturating_add(4); // Title also needs borders and padding
        let required_width = required_width_content.max(required_width_title);
        let width = required_width.max(min_width).min(max_width);

        // Calculate height based on content wrapping
        // Use content_width for wrapping calculation (without borders/padding)
        let text_area_width = width.saturating_sub(4); // Subtract borders and padding for text area
        let height = Self::calculate_wrapped_lines(&display_message, text_area_width as usize) as u16 + 2; // +2 for borders

        (width, height)
    }

    /// Create a top-right positioned rectangle with absolute dimensions
    pub fn top_right_rect_absolute(width: u16, height: u16, r: Rect) -> Rect {
        let x = r.width.saturating_sub(width.saturating_add(1)); // -1 for right margin
        let y = 1; // Top margin

        Rect {
            x: r.x + x,
            y: r.y + y,
            width: width.min(r.width),
            height: height.min(r.height),
        }
    }

    /// Create a centered rectangle with percentage-based sizing
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    /// Get toast display message with formatting
    pub fn get_toast_display_message(toast_state: &ToastState) -> String {
        toast_state.message.clone()
    }

    /// Calculate number of lines needed for text when wrapped to given width
    pub fn calculate_wrapped_lines(text: &str, width: usize) -> usize {
        if width == 0 {
            return text.lines().count().max(1);
        }

        text.lines()
            .map(|line| {
                if line.is_empty() {
                    1
                } else {
                    // Simple character-based wrapping (could be improved with Unicode awareness)
                    let char_count = line.chars().count();
                    (char_count + width - 1) / width // Ceiling division
                }
            })
            .sum::<usize>()
            .max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_wrapped_lines() {
        // Test empty string
        assert_eq!(TuiRenderer::calculate_wrapped_lines("", 10), 1);

        // Test single line, no wrapping needed
        assert_eq!(TuiRenderer::calculate_wrapped_lines("hello", 10), 1);

        // Test single line, wrapping needed
        assert_eq!(TuiRenderer::calculate_wrapped_lines("hello world", 5), 3); // "hello" + " worl" + "d"

        // Test multiple lines
        assert_eq!(TuiRenderer::calculate_wrapped_lines("line1\nline2", 10), 2);

        // Test zero width (edge case)
        assert!(TuiRenderer::calculate_wrapped_lines("test", 0) > 0);

        // Test realistic toast message
        let long_message = "Indexing completed: 25 files, 1200 symbols found successfully";
        assert!(TuiRenderer::calculate_wrapped_lines(long_message, 30) >= 2);
    }

    #[test]
    fn test_toast_size_calculation() {
        let mut toast_state = ToastState::new();
        toast_state.show(
            "Test message".to_string(),
            ToastType::Info,
            std::time::Duration::from_secs(3),
        );

        let screen_size = Rect::new(0, 0, 80, 24);
        let (width, height) = TuiRenderer::calculate_toast_size_absolute(&toast_state, screen_size);

        assert!(width >= 20); // Minimum width
        assert!(width <= 76); // Max width (80 - 4 for margins)
        assert!(height >= 2); // At least borders
    }

    #[test]
    fn test_top_right_rect_positioning() {
        let screen = Rect::new(0, 0, 80, 24);
        let popup = TuiRenderer::top_right_rect_absolute(20, 5, screen);

        // Should be positioned at top-right
        assert_eq!(popup.x, 59); // 80 - 20 - 1
        assert_eq!(popup.y, 1); // Top margin
        assert_eq!(popup.width, 20);
        assert_eq!(popup.height, 5);
    }

    #[test]
    fn test_centered_rect() {
        let screen = Rect::new(0, 0, 100, 50);
        let popup = TuiRenderer::centered_rect(50, 30, screen);

        // Should be centered
        assert_eq!(popup.width, 50);
        assert_eq!(popup.height, 15); // 30% of 50

        // Position should be centered
        assert_eq!(popup.x, 25); // (100 - 50) / 2
        assert_eq!(popup.y, 17); // Centered vertically
    }
}
