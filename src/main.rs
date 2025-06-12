use fae::cli;
use std::process;

fn main() {
    if let Err(err) = cli::run_cli() {
        eprintln!("Error: {}", err);
        
        // エラーチェーンを表示
        let mut source = err.source();
        while let Some(err) = source {
            eprintln!("Caused by: {}", err);
            source = err.source();
        }
        
        process::exit(1);
    }
}