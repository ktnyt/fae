//! TUI (Terminal User Interface) モジュール
//! 
//! リファクタリング後の構造化TUIモジュール

pub mod constants;
pub mod styles;
pub mod text_editing;
pub mod input_handler;

// 公開API
pub use constants::*;
pub use styles::TuiStyles;
pub use text_editing::{TextEditor, EditableText};
pub use input_handler::{InputHandler, InputResult, NavigationAction};

// 元のTUI実装も継続して公開（段階的移行）
mod engine;
pub use engine::*;