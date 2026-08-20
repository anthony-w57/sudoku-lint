use std::env;
use std::fs;
use std::process::ExitCode;

use sudoku_lint::{lint, Severity};

fn main() -> ExitCode {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "sudoku-lint".to_string());
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: {} <board-file>", program);
            return ExitCode::FAILURE;
        }
    };

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{}: {}", path, err);
            return ExitCode::FAILURE;
        }
    };

    let findings = lint(&source);
    let mut had_error = false;
    for finding in &findings {
        if finding.severity == Severity::Error {
            had_error = true;
        }
        println!(
            "{}:{}: {}: {}",
            path, finding.line, finding.severity, finding.message
        );
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
