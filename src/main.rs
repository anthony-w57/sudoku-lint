use std::env;
use std::fs;
use std::process::ExitCode;

use sudoku_lint::{findings_to_json, lint, Severity};

fn main() -> ExitCode {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "sudoku-lint".to_string());

    let mut json = false;
    let mut path = None;
    for arg in args {
        if arg == "--json" {
            json = true;
        } else {
            path = Some(arg);
        }
    }
    let path = match path {
        Some(path) => path,
        None => {
            eprintln!("usage: {} [--json] <board-file>", program);
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
    let had_error = findings.iter().any(|f| f.severity == Severity::Error);

    if json {
        println!("{}", findings_to_json(&findings));
    } else {
        for finding in &findings {
            println!(
                "{}:{}: {}: {}",
                path, finding.line, finding.severity, finding.message
            );
        }
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
