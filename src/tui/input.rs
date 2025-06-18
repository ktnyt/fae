//! Input handling and operations
//!
//! Provides unified input processing with support for Emacs-style key bindings
//! and text editing operations. All input operations are defined as a single
//! enum for type safety and consistent handling.

/// Input operations for unified input handling
#[derive(Debug, Clone)]
pub enum InputOperation {
    InsertChar(char),
    MoveCursorToStart,
    MoveCursorToEnd,
    MoveCursorLeft,
    MoveCursorRight,
    DeleteCharForward,
    DeleteCharBackward,
    KillLine,
    Yank,
}

/// Input handler for managing text editing state
pub struct InputHandler;

impl InputHandler {
    /// Apply an input operation to the given text and cursor state
    pub fn apply_operation(
        operation: InputOperation,
        text: &mut String,
        cursor_position: &mut usize,
        kill_ring: &mut String,
    ) {
        match operation {
            InputOperation::InsertChar(c) => {
                text.insert(*cursor_position, c);
                *cursor_position += 1;
            }
            InputOperation::MoveCursorToStart => {
                *cursor_position = 0;
            }
            InputOperation::MoveCursorToEnd => {
                *cursor_position = text.len();
            }
            InputOperation::MoveCursorLeft => {
                if *cursor_position > 0 {
                    *cursor_position -= 1;
                }
            }
            InputOperation::MoveCursorRight => {
                if *cursor_position < text.len() {
                    *cursor_position += 1;
                }
            }
            InputOperation::DeleteCharForward => {
                if *cursor_position < text.len() {
                    text.remove(*cursor_position);
                }
            }
            InputOperation::DeleteCharBackward => {
                if *cursor_position > 0 {
                    *cursor_position -= 1;
                    text.remove(*cursor_position);
                }
            }
            InputOperation::KillLine => {
                if *cursor_position < text.len() {
                    let killed_text = text[*cursor_position..].to_string();
                    *kill_ring = killed_text;
                    text.truncate(*cursor_position);
                }
            }
            InputOperation::Yank => {
                if !kill_ring.is_empty() {
                    let insert_text = kill_ring.clone();
                    text.insert_str(*cursor_position, &insert_text);
                    *cursor_position += insert_text.len();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_operations() {
        let mut text = "hello".to_string();
        let mut cursor = 2; // Between 'e' and 'l'
        let mut kill_ring = String::new();

        // Test insert character
        InputHandler::apply_operation(
            InputOperation::InsertChar('X'),
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(text, "heXllo");
        assert_eq!(cursor, 3);

        // Test move to start
        InputHandler::apply_operation(
            InputOperation::MoveCursorToStart,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 0);

        // Test move to end
        InputHandler::apply_operation(
            InputOperation::MoveCursorToEnd,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 6); // Length of "heXllo"
    }

    #[test]
    fn test_kill_and_yank() {
        let mut text = "hello world".to_string();
        let mut cursor = 6; // After "hello "
        let mut kill_ring = String::new();

        // Kill line (from cursor to end)
        InputHandler::apply_operation(
            InputOperation::KillLine,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(text, "hello ");
        assert_eq!(kill_ring, "world");
        assert_eq!(cursor, 6);

        // Move cursor back and yank
        cursor = 5; // Before the space
        InputHandler::apply_operation(InputOperation::Yank, &mut text, &mut cursor, &mut kill_ring);
        assert_eq!(text, "helloworld ");
        assert_eq!(cursor, 10); // After "helloworld"
    }

    #[test]
    fn test_cursor_movement() {
        let mut text = "test".to_string();
        let mut cursor = 2;
        let mut kill_ring = String::new();

        // Move left
        InputHandler::apply_operation(
            InputOperation::MoveCursorLeft,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 1);

        // Move right
        InputHandler::apply_operation(
            InputOperation::MoveCursorRight,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 2);

        // Test boundary conditions - move left at start
        cursor = 0;
        InputHandler::apply_operation(
            InputOperation::MoveCursorLeft,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 0); // Should remain at 0

        // Test boundary conditions - move right at end
        cursor = text.len();
        InputHandler::apply_operation(
            InputOperation::MoveCursorRight,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(cursor, 4); // Should remain at end
    }

    #[test]
    fn test_delete_operations() {
        let mut text = "hello".to_string();
        let mut cursor = 2; // Between 'e' and 'l'
        let mut kill_ring = String::new();

        // Delete character forward (C-d)
        InputHandler::apply_operation(
            InputOperation::DeleteCharForward,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(text, "helo");
        assert_eq!(cursor, 2); // Cursor stays in place

        // Delete character backward (C-h, Backspace)
        InputHandler::apply_operation(
            InputOperation::DeleteCharBackward,
            &mut text,
            &mut cursor,
            &mut kill_ring,
        );
        assert_eq!(text, "hlo");
        assert_eq!(cursor, 1); // Cursor moves back
    }
}
