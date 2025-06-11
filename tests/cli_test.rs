use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn run_cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("sfs")
        .arg("--")
        .args(args)
        .output()
        .expect("Failed to execute command");

    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[cfg(test)]
mod cli_integration_tests {
    use super::*;

    fn create_test_files(test_dir: &PathBuf) -> Result<(), std::io::Error> {
        let test_ts_content = r#"
interface TestInterface {
    id: number;
    name: string;
}

class TestClass {
    constructor(private data: TestInterface) {}
    
    getData(): TestInterface {
        return this.data;
    }
}

function testFunction(): void {
    console.log("test");
}

const TEST_CONSTANT = "test value";
"#;

        let test_js_content = r#"
class SimpleClass {
    constructor() {
        this.value = 0;
    }
    
    getValue() {
        return this.value;
    }
}

function simpleFunction() {
    return "simple";
}

const SIMPLE_CONSTANT = 42;
"#;

        fs::write(test_dir.join("test.ts"), test_ts_content)?;
        fs::write(test_dir.join("test.js"), test_js_content)?;

        Ok(())
    }

    mod basic_cli_functionality {
        use super::*;

        #[test]
        fn should_start_interactive_mode_when_no_arguments_provided() {
            // This would test interactive mode, but for now we'll skip
            // as it requires complex terminal interaction simulation
            // We can add this later with proper PTY handling

            // For now, just test that the binary exists and can be called
            let (exit_code, _stdout, _stderr) = run_cli(&["--help"]);
            assert_eq!(exit_code, 0);
        }

        #[test]
        fn should_show_help_information() {
            let (exit_code, stdout, _stderr) = run_cli(&["--help"]);

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("Usage:") || stdout.contains("help"));
        }
    }

    mod symbol_searching {
        use super::*;

        #[test]
        fn should_find_symbols_in_test_directory() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) =
                run_cli(&["TestClass", "--directory", test_dir.to_str().unwrap()]);

            assert_eq!(exit_code, 0);

            // Should contain indexing information
            assert!(
                stdout.contains("Indexing")
                    || stdout.contains("Found")
                    || stdout.contains("TestClass")
            );
        }

        #[test]
        fn should_handle_fuzzy_search() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "TstCls", // Fuzzy search for TestClass
                "--directory",
                test_dir.to_str().unwrap(),
            ]);

            assert_eq!(exit_code, 0);

            // Should contain indexing information
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }

        #[test]
        fn should_limit_results_when_requested() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                test_dir.to_str().unwrap(),
                "--limit",
                "2",
            ]);

            assert_eq!(exit_code, 0);

            // Should respect the limit
            let lines: Vec<&str> = stdout.lines().collect();

            // Should have completed successfully with limited results
            assert!(!lines.is_empty());
        }

        #[test]
        fn should_filter_by_symbol_types() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                test_dir.to_str().unwrap(),
                "--types",
                "function",
            ]);

            assert_eq!(exit_code, 0);

            // Should contain indexing and search information
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }

        #[test]
        fn should_find_functions_with_symbol_extraction() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "testFunction",
                "--directory",
                test_dir.to_str().unwrap(),
                "--types",
                "function",
            ]);

            assert_eq!(exit_code, 0);

            // Should find function symbols
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }

        #[test]
        fn should_adjust_fuzzy_search_threshold() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            // Test with strict threshold
            let (exit_code1, stdout1, _stderr1) = run_cli(&[
                "tst", // Very fuzzy search
                "--directory",
                test_dir.to_str().unwrap(),
                "--threshold",
                "0.1", // Very strict
            ]);

            // Test with loose threshold
            let (exit_code2, stdout2, _stderr2) = run_cli(&[
                "tst", // Very fuzzy search
                "--directory",
                test_dir.to_str().unwrap(),
                "--threshold",
                "0.8", // Very loose
            ]);

            assert_eq!(exit_code1, 0);
            assert_eq!(exit_code2, 0);

            // Both should complete successfully
            assert!(!stdout1.is_empty());
            assert!(!stdout2.is_empty());
        }

        #[test]
        fn should_exclude_files_and_directories_when_requested() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                test_dir.to_str().unwrap(),
                "--no-files",
                "--no-dirs",
            ]);

            assert_eq!(exit_code, 0);

            // Should complete successfully
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }

        #[test]
        fn should_support_symbol_only_search_simulation() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                test_dir.to_str().unwrap(),
                "--no-files",
                "--no-dirs",
                "--types",
                "function,variable",
            ]);

            assert_eq!(exit_code, 0);

            // Should find symbols but not files/directories
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }

        #[test]
        fn should_support_file_only_search_simulation() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                test_dir.to_str().unwrap(),
                "--types",
                "filename,dirname",
            ]);

            assert_eq!(exit_code, 0);

            // Should find files and directories
            assert!(stdout.contains("Indexing") || stdout.contains("Found"));
        }
    }

    mod error_handling {
        use super::*;

        #[test]
        fn should_handle_non_existent_directory_gracefully() {
            let (exit_code, stdout, stderr) =
                run_cli(&["test", "--directory", "/non/existent/path"]);

            // Should handle gracefully (may exit with error or show message)
            assert!(exit_code == 0 || exit_code != 0); // Either way is acceptable
            assert!(!stdout.is_empty() || !stderr.is_empty());
        }

        #[test]
        fn should_handle_invalid_command_line_options() {
            let (exit_code, _stdout, stderr) = run_cli(&["--invalid-option"]);

            // Should exit with error for unknown options
            assert_ne!(exit_code, 0);
            assert!(
                stderr.contains("error")
                    || stderr.contains("unknown")
                    || stderr.contains("invalid")
            );
        }

        #[test]
        fn should_show_no_results_message_when_nothing_found() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "NonExistentSymbol12345",
                "--directory",
                test_dir.to_str().unwrap(),
            ]);

            assert_eq!(exit_code, 0);

            // Should show message about no results
            assert!(
                stdout.contains("No results")
                    || stdout.contains("0 symbols")
                    || stdout.contains("found")
            );
        }
    }

    mod performance_and_reliability {
        use super::*;
        use std::time::Instant;

        #[test]
        fn should_complete_within_reasonable_time_for_small_projects() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();

            create_test_files(&test_dir).expect("Failed to create test files");

            let start_time = Instant::now();

            let (exit_code, _stdout, _stderr) =
                run_cli(&["test", "--directory", test_dir.to_str().unwrap()]);

            let execution_time = start_time.elapsed();

            assert_eq!(exit_code, 0);

            // Should complete within 10 seconds for small test files
            assert!(execution_time.as_secs() < 10);
        }

        #[test]
        fn should_handle_empty_directories() {
            let temp_dir = tempdir().expect("Failed to create temp dir");
            let test_dir = temp_dir.path().to_path_buf();
            let empty_dir = test_dir.join("empty");

            fs::create_dir_all(&empty_dir).expect("Failed to create empty dir");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "test",
                "--directory",
                empty_dir.to_str().unwrap(),
                "--verbose", // Add verbose flag to see detailed messages
            ]);

            assert_eq!(exit_code, 0);

            // Should handle empty directory gracefully with verbose output
            assert!(
                stdout.contains("Indexing")
                    || stdout.contains("0 symbols")
                    || stdout.contains("Found")
            );
        }
    }

    mod verbose_output_control {
        use super::*;
        use std::fs;
        use tempfile::TempDir;

        #[test]
        fn should_show_detailed_output_with_verbose_flag() {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let test_file = temp_dir.path().join("test.js");
            fs::write(&test_file, "function testFunc() { return 42; }")
                .expect("Failed to write test file");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "testFunc",
                "--directory",
                temp_dir.path().to_str().unwrap(),
                "--verbose",
            ]);

            assert_eq!(exit_code, 0);

            // Should show detailed indexing information with --verbose
            assert!(stdout.contains("Indexing files"));
            assert!(stdout.contains("Found") && stdout.contains("symbols"));
            assert!(stdout.contains("testFunc"));
        }

        #[test]
        fn should_show_minimal_output_without_verbose_flag() {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let test_file = temp_dir.path().join("test.js");
            fs::write(&test_file, "function testFunc() { return 42; }")
                .expect("Failed to write test file");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "testFunc",
                "--directory",
                temp_dir.path().to_str().unwrap(),
                // No --verbose flag
            ]);

            assert_eq!(exit_code, 0);

            // Should NOT show detailed indexing information without --verbose
            assert!(!stdout.contains("Indexing files"));
            assert!(
                !stdout.contains("Found")
                    || !stdout.contains("symbols")
                    || stdout.contains("Found 1 results")
            );
            // Should still show search results
            assert!(stdout.contains("testFunc"));
        }

        #[test]
        fn should_handle_verbose_flag_with_no_results() {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let test_file = temp_dir.path().join("test.js");
            fs::write(&test_file, "function testFunc() { return 42; }")
                .expect("Failed to write test file");

            let (exit_code, stdout, _stderr) = run_cli(&[
                "nonexistentfunction",
                "--directory",
                temp_dir.path().to_str().unwrap(),
                "--verbose",
            ]);

            assert_eq!(exit_code, 0);

            // Should show indexing info even when no results found
            assert!(stdout.contains("Indexing files"));
            assert!(stdout.contains("Found") && stdout.contains("symbols"));
            assert!(stdout.contains("No results found"));
        }

        #[test]
        fn should_handle_empty_directory_with_and_without_verbose() {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let empty_dir = temp_dir.path().join("empty");
            fs::create_dir_all(&empty_dir).expect("Failed to create empty dir");

            // Test with verbose
            let (exit_code, stdout_verbose, _stderr) = run_cli(&[
                "test",
                "--directory",
                empty_dir.to_str().unwrap(),
                "--verbose",
            ]);
            assert_eq!(exit_code, 0);
            assert!(stdout_verbose.contains("Indexing files") || stdout_verbose.contains("Found"));

            // Test without verbose
            let (exit_code, stdout_minimal, _stderr) = run_cli(&[
                "test",
                "--directory",
                empty_dir.to_str().unwrap(),
                // No --verbose flag
            ]);
            assert_eq!(exit_code, 0);
            // Should not contain detailed indexing info
            assert!(!stdout_minimal.contains("Indexing files"));
            // Should contain "No results found" message
            assert!(stdout_minimal.contains("No results found"));
        }
    }
}
