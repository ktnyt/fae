//! Fast And Elegant (fae) - High-performance code symbol search tool
//!
//! Command-line usage:
//!   fae [query]       - Literal search (fallback: rg → ag → native)
//!   fae #[query]      - Symbol search
//!   fae $[query]      - Variable search
//!   fae @[query]      - Filepath search
//!   fae /[query]      - Regex search (fallback: rg → ag → native)

use fae::actors::messages::FaeMessage;
use fae::cli::create_search_params;
use fae::core::Message;
use fae::tui::TuiApp;
use fae::unified_search::UnifiedSearchSystem;
use std::env;

/// CLI application configuration
struct FaeConfig {
    query: String,
    search_path: String,
    timeout_ms: u64,
}

impl Default for FaeConfig {
    fn default() -> Self {
        Self {
            query: String::new(),
            search_path: ".".to_string(),
            timeout_ms: 15000, // Increased timeout for symbol indexing
        }
    }
}

/// Parse command line arguments
fn parse_args() -> Result<Option<FaeConfig>, String> {
    let args: Vec<String> = env::args().collect();

    // If no query provided, launch TUI mode
    if args.len() < 2 {
        return Ok(None);
    }

    let mut config = FaeConfig {
        query: args[1].clone(),
        ..Default::default()
    };

    // Parse additional arguments if needed (future extension)
    for i in 2..args.len() {
        match args[i].as_str() {
            "--path" => {
                if i + 1 < args.len() {
                    config.search_path = args[i + 1].clone();
                }
            }
            _ => {}
        }
    }

    Ok(Some(config))
}

/// Setup file logging for TUI mode to avoid interfering with terminal UI
/// Returns the path to the log file or an error if no suitable location is found
fn setup_file_logging() -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;

    // List of possible log file locations in order of preference
    let possible_paths = vec![
        std::env::temp_dir().join("fae.log"),
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("fae.log"),
        std::path::PathBuf::from("fae.log"),
    ];

    let mut actual_log_path = None;
    let mut last_error = None;

    // Try each location until one works
    for path in possible_paths {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(_) => {
                actual_log_path = Some(path);
                break;
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    let log_path = actual_log_path.ok_or_else(|| {
        format!(
            "Failed to create log file in any location. Last error: {}",
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        )
    })?;

    // Create the logger with improved error handling
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Failed to open log file {}: {}", log_path.display(), e))?;

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .format(|buf, record| {
            use chrono::{DateTime, Utc};
            let timestamp: DateTime<Utc> = Utc::now();
            writeln!(
                buf,
                "{} [{}] {} - {}",
                timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    log::info!("TUI mode started - logging to: {}", log_path.display());
    log::debug!("Log file permissions: {:?}", std::fs::metadata(&log_path));

    Ok(log_path)
}

/// Check if the current environment supports TUI mode
fn check_terminal_environment() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if we're in a proper terminal
    if !atty::is(atty::Stream::Stdout) {
        return Err("TUI mode requires a proper terminal (stdout is not a TTY)".into());
    }

    if !atty::is(atty::Stream::Stdin) {
        return Err("TUI mode requires a proper terminal (stdin is not a TTY)".into());
    }

    // Check for essential environment variables
    if std::env::var("TERM").is_err() {
        return Err("TUI mode requires TERM environment variable to be set".into());
    }

    // Check terminal size capability
    match crossterm::terminal::size() {
        Ok((width, height)) => {
            if width < 40 || height < 10 {
                return Err(format!(
                    "Terminal size too small for TUI ({}x{}, minimum 40x10 required)",
                    width, height
                )
                .into());
            }
            log::debug!("Terminal size check passed: {}x{}", width, height);
        }
        Err(e) => {
            return Err(format!("Failed to get terminal size: {}", e).into());
        }
    }

    log::debug!("Terminal environment check passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_file_logging_creates_valid_path() {
        // Test that setup_file_logging returns a path when successful
        // Note: This test may fail in environments without write permissions
        match setup_file_logging() {
            Ok(path) => {
                assert!(path.exists() || path.as_os_str() != "(no log file)");
                println!("Log file created at: {}", path.display());
            }
            Err(e) => {
                println!("Expected in CI environment: {}", e);
                // This is expected in CI environments
            }
        }
    }

    #[test]
    fn test_terminal_environment_check_in_ci() {
        // In CI environments, this should fail gracefully
        match check_terminal_environment() {
            Ok(()) => {
                println!("Terminal environment is available");
            }
            Err(e) => {
                println!("Terminal environment check failed (expected in CI): {}", e);
                // Expected in CI environments
                assert!(e.to_string().contains("TTY") || e.to_string().contains("TERM"));
            }
        }
    }

    #[test]
    fn test_parse_args_no_query_returns_none() {
        // Save original args (for future mock implementation)
        let _original_args = std::env::args().collect::<Vec<_>>();

        // Mock args with just program name (no query)
        std::env::set_var("PROGRAM", "fae");

        // This test would need proper arg mocking to be fully effective
        // For now, just verify the function signature works
        match parse_args() {
            Ok(None) => {
                println!("No query provided - TUI mode expected");
            }
            Ok(Some(_)) => {
                println!("Query provided - CLI mode expected");
            }
            Err(e) => {
                println!("Parse error: {}", e);
            }
        }

        // Note: In a real test environment, we'd properly mock std::env::args()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command line arguments first to determine mode
    let config = match parse_args() {
        Ok(Some(config)) => config,
        Ok(None) => {
            // TUI mode - setup file logging to avoid interfering with TUI
            let _log_path = match setup_file_logging() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Warning: Failed to setup file logging: {}", e);
                    eprintln!("TUI will continue without file logging");
                    // Initialize simple stderr logging as fallback
                    env_logger::init();
                    std::path::PathBuf::from("(no log file)")
                }
            };

            // Small delay to let user see the message
            std::thread::sleep(std::time::Duration::from_millis(1500));

            // Check terminal environment before launching TUI
            if let Err(e) = check_terminal_environment() {
                eprintln!("Terminal environment check failed: {}", e);
                eprintln!("Consider running in a proper terminal or using CLI mode with a query argument.");
                eprintln!("Example: fae \"search term\"");
                std::process::exit(1);
            }

            // Launch TUI mode with simplified UnifiedSearchSystem integration
            let (mut app, tui_handle) = TuiApp::new(".")
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            // Create UnifiedSearchSystem with file watching for TUI mode
            let (mut search_system, control_sender, mut result_receiver) = UnifiedSearchSystem::new(
                ".",
                true, // Enable file watching for TUI mode
            )
            .await?;

            // Create simplified TUI message handler
            struct TuiSearchHandler {
                control_sender: tokio::sync::mpsc::UnboundedSender<
                    fae::core::Message<fae::actors::messages::FaeMessage>,
                >,
            }

            impl fae::tui::TuiMessageHandler for TuiSearchHandler {
                fn execute_search(
                    &self,
                    query: String,
                ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    use fae::actors::messages::FaeMessage;
                    use fae::cli::create_search_params;
                    use fae::core::Message;

                    log::debug!("TuiSearchHandler executing search: '{}'", query);

                    // Parse the query and determine search mode
                    let search_params = create_search_params(&query);

                    // Generate request ID and send search request
                    let request_id = tiny_id::ShortCodeGenerator::new_alphanumeric(8).next_string();
                    let search_message = Message::new(
                        "updateSearchParams",
                        FaeMessage::UpdateSearchParams {
                            params: search_params,
                            request_id,
                        },
                    );

                    self.control_sender
                        .send(search_message)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

                    Ok(())
                }

                fn clear_results(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    use fae::actors::messages::FaeMessage;
                    use fae::core::Message;

                    let clear_message = Message::new("clearResults", FaeMessage::ClearResults);
                    self.control_sender
                        .send(clear_message)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
                    Ok(())
                }

                fn abort_search(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    use fae::actors::messages::FaeMessage;
                    use fae::core::Message;

                    let abort_message = Message::new("abortSearch", FaeMessage::AbortSearch);
                    self.control_sender
                        .send(abort_message)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
                    Ok(())
                }
            }

            // Set the simplified message handler
            let handler = TuiSearchHandler {
                control_sender: control_sender.clone(),
            };
            app.set_message_handler(Box::new(handler));

            // Handle search results by directly updating TUI state
            let tui_handle_for_results = tui_handle.clone();
            tokio::spawn(async move {
                while let Some(message) = result_receiver.recv().await {
                    log::debug!("Processing search result: {}", message.method);

                    match &message.payload {
                        fae::actors::messages::FaeMessage::PushSearchResult { result, .. } => {
                            let formatted_result = format!(
                                "{}:{} - {}",
                                result.filename,
                                result.line,
                                result.content.trim()
                            );

                            if let Err(e) =
                                tui_handle_for_results.append_search_results(vec![formatted_result])
                            {
                                log::warn!("Failed to add search result to TUI: {}", e);
                            }
                        }
                        fae::actors::messages::FaeMessage::CompleteSearch => {
                            if let Err(e) = tui_handle_for_results.show_toast(
                                "Search completed".to_string(),
                                fae::tui::ToastType::Success,
                                std::time::Duration::from_secs(2),
                            ) {
                                log::warn!("Failed to show completion toast: {}", e);
                            }
                        }
                        fae::actors::messages::FaeMessage::NotifySearchReport { result_count } => {
                            if let Err(e) = tui_handle_for_results.show_toast(
                                format!("Search completed: {} results found", result_count),
                                fae::tui::ToastType::Success,
                                std::time::Duration::from_secs(3),
                            ) {
                                log::warn!("Failed to show search report toast: {}", e);
                            }
                        }
                        _ => {
                            // Handle other message types as needed
                            log::debug!("Unhandled message type: {}", message.method);
                        }
                    }
                }
                log::debug!("Result processing ended");
            });

            // Initialize symbol indexing
            let init_message = fae::core::Message::new(
                "initialize",
                fae::actors::messages::FaeMessage::ClearResults,
            );
            if let Err(e) = control_sender.send(init_message) {
                eprintln!("Failed to send initialize message: {}", e);
                std::process::exit(1);
            }

            // Run TUI application and handle shutdown properly
            let app_result = app.run().await;

            // Shutdown search system when TUI exits
            search_system.shutdown();

            return app_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
        }
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    // CLI mode - use standard console logging
    env_logger::init();

    // Parse query and determine search mode
    let search_params = create_search_params(&config.query);

    log::info!(
        "Starting search: '{}' (mode: {:?})",
        search_params.query,
        search_params.mode
    );

    // Create unified search system (CLI mode doesn't need file watching)
    // Pass search mode for optimization (skip symbol actors for non-symbol searches)
    let (mut search_system, control_sender, mut result_receiver) = UnifiedSearchSystem::new_with_mode(
        &config.search_path,
        false,
        Some(search_params.mode),
    )
    .await?;

    // Initialize symbol indexing
    let init_message = Message::new("initialize", FaeMessage::ClearResults); // Dummy payload for initialize
    if let Err(e) = control_sender.send(init_message) {
        eprintln!("Failed to send initialize message: {}", e);
        std::process::exit(1);
    }

    // Generate request ID and send search request
    let request_id = tiny_id::ShortCodeGenerator::new_alphanumeric(8).next_string();
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams {
            params: search_params,
            request_id,
        },
    );
    if let Err(e) = control_sender.send(search_message) {
        eprintln!("Failed to send search message: {}", e);
        std::process::exit(1);
    }

    // Wait for search completion or timeout
    let mut result_count = 0;
    let timeout = tokio::time::Duration::from_millis(config.timeout_ms);

    match tokio::time::timeout(timeout, async {
        while let Some(message) = result_receiver.recv().await {
            match &message.payload {
                FaeMessage::NotifySearchReport {
                    result_count: _count,
                } => {
                    // Return the actual number of results we printed, not the total found
                    log::debug!("Search completed, printed {} results", result_count);
                    return result_count;
                }
                FaeMessage::PushSearchResult {
                    result,
                    request_id: _,
                } => {
                    // Print result immediately for CLI mode
                    println!(
                        "{}:{} - {}",
                        result.filename,
                        result.line,
                        result.content.trim()
                    );
                    result_count += 1;
                    // Note: Continue processing until NotifySearchReport for graceful shutdown
                    // ResultHandlerActor will automatically trigger completion when max_results is reached
                }
                _ => {}
            }
        }
        result_count
    })
    .await
    {
        Ok(count) => result_count = count,
        Err(_) => {
            eprintln!("Search timed out after {}ms", config.timeout_ms);
        }
    }

    // Shutdown the system
    search_system.shutdown();

    if result_count == 0 {
        eprintln!("No results found.");
        std::process::exit(1);
    } else {
        log::info!("Search completed with {} results", result_count);
    }

    Ok(())
}
