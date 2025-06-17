//! Demo of Actor-based TUI implementation
//!
//! This example demonstrates the Actor-based TUI architecture where
//! TuiHandler receives messages from UnifiedSearchSystem and updates
//! the terminal display accordingly.

use fae::tui::TuiApp;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments or use current directory
    let search_path = env::args().nth(1).unwrap_or_else(|| ".".to_string());

    println!("Starting Actor-based TUI demo...");
    println!("Search path: {}", search_path);
    println!("Press 'q' or Esc to quit");

    // Create and run Actor-based TUI application
    let mut app = TuiApp::new(&search_path).await?;
    app.run().await?;

    println!("Actor TUI demo completed.");
    Ok(())
}
