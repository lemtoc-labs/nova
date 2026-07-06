#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use nova::config::load::load_config;
use nova::render::render;
use nova::state::{Keymap, PromptState};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("prompt") => run_prompt(&args[1..]),
        Some("check") => run_check(&args[1..]),
        Some("init") | Some("worker") => not_implemented(),
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

fn run_prompt(args: &[String]) -> ExitCode {
    match PromptArgs::parse(args).and_then(|args| args.render()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nova: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_check(args: &[String]) -> ExitCode {
    match CheckArgs::parse(args).and_then(|args| args.check()) {
        Ok(()) => {
            println!("nova: config ok");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nova: {error}");
            ExitCode::FAILURE
        }
    }
}

fn not_implemented() -> ExitCode {
    eprintln!("nova: command is not implemented yet");
    ExitCode::FAILURE
}

fn print_help() {
    println!("Usage: nova <init|worker|prompt|check>");
}

#[derive(Clone, Debug)]
struct PromptArgs {
    cwd: PathBuf,
    columns: u16,
    exit_status: i32,
    duration_ms: Option<u64>,
    keymap: Keymap,
    config_path: Option<PathBuf>,
    format: PromptFormat,
}

impl PromptArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            cwd: env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?,
            columns: env::var("COLUMNS")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(80),
            exit_status: 0,
            duration_ms: None,
            keymap: Keymap::Main,
            config_path: None,
            format: PromptFormat::Plain,
        };

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--cwd" => {
                    parsed.cwd = PathBuf::from(required_value(args, index, "--cwd")?);
                    index += 2;
                }
                "--cols" => {
                    parsed.columns = parse_value(required_value(args, index, "--cols")?, "--cols")?;
                    index += 2;
                }
                "--exit" => {
                    parsed.exit_status =
                        parse_value(required_value(args, index, "--exit")?, "--exit")?;
                    index += 2;
                }
                "--duration-ms" => {
                    parsed.duration_ms = Some(parse_value(
                        required_value(args, index, "--duration-ms")?,
                        "--duration-ms",
                    )?);
                    index += 2;
                }
                "--keymap" => {
                    parsed.keymap = Keymap::parse(required_value(args, index, "--keymap")?);
                    index += 2;
                }
                "--config" => {
                    parsed.config_path =
                        Some(PathBuf::from(required_value(args, index, "--config")?));
                    index += 2;
                }
                "--format" => {
                    parsed.format = PromptFormat::parse(required_value(args, index, "--format")?)?;
                    index += 2;
                }
                "--help" | "-h" => {
                    return Err("usage: nova prompt [--cwd PATH] [--cols N] [--exit N] [--duration-ms N] [--keymap KEYMAP] [--config PATH] [--format plain|shell]".to_string());
                }
                option => return Err(format!("unknown prompt option `{option}`")),
            }
        }

        Ok(parsed)
    }

    fn render(self) -> Result<String, String> {
        let config = load_config(self.config_path.as_deref()).map_err(|error| error.to_string())?;
        let output = render(
            &config,
            &PromptState {
                cwd: self.cwd,
                exit_status: self.exit_status,
                duration_ms: self.duration_ms,
                columns: self.columns,
                keymap: self.keymap,
            },
        );

        Ok(match self.format {
            PromptFormat::Plain => {
                if output.rprompt.is_empty() {
                    format!("{}\n", output.prompt)
                } else {
                    format!("{}\n{}\n", output.prompt, output.rprompt)
                }
            }
            PromptFormat::Shell => format!(
                "PROMPT={}\nRPROMPT={}\n",
                zsh_quote(&output.prompt),
                zsh_quote(&output.rprompt)
            ),
        })
    }
}

#[derive(Clone, Debug)]
struct CheckArgs {
    config_path: Option<PathBuf>,
}

impl CheckArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self { config_path: None };
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--config" => {
                    parsed.config_path =
                        Some(PathBuf::from(required_value(args, index, "--config")?));
                    index += 2;
                }
                option => return Err(format!("unknown check option `{option}`")),
            }
        }

        Ok(parsed)
    }

    fn check(self) -> Result<(), String> {
        load_config(self.config_path.as_deref())
            .map(|_config| ())
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
enum PromptFormat {
    Plain,
    Shell,
}

impl PromptFormat {
    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "plain" => Ok(Self::Plain),
            "shell" => Ok(Self::Shell),
            _ => Err(format!("unknown prompt format `{input}`")),
        }
    }
}

fn required_value<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_value<T>(value: &str, option: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| format!("invalid value for {option}: {error}"))
}

fn zsh_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}
