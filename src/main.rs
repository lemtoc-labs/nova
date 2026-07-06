#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("init") | Some("worker") | Some("prompt") | Some("check") => {
            eprintln!("nova: command is not implemented yet");
            ExitCode::FAILURE
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("nova: unknown command `{command}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("Usage: nova <init|worker|prompt|check>");
}
