#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use nova::config::error::ConfigWarning;
use nova::config::load::load_config;
use nova::render::render;
use nova::segments::known_segment_ids;
use nova::state::{Keymap, PromptState};
use nova::worker::{WorkerOptions, run as run_worker_loop};
use nova::zsh_init::render_init_script;

const PROMPT_USAGE: &str = "Usage: nova prompt [--cwd PATH] [--cols N] [--exit N] [--duration-ms N] [--time HH:MM:SS] [--keymap KEYMAP] [--config PATH] [--format plain|preview|shell]";

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("init") => run_init(&args[1..]),
        Some("prompt") => run_prompt(&args[1..]),
        Some("check") => run_check(&args[1..]),
        Some("worker") => run_worker(&args[1..]),
        Some("--version") | Some("-V") => {
            println!("nova {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
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

fn run_init(args: &[String]) -> ExitCode {
    match args {
        [shell] if shell == "zsh" => match env::current_exe() {
            Ok(path) => {
                print!("{}", render_init_script(&path));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("nova: failed to locate current executable: {error}");
                ExitCode::FAILURE
            }
        },
        [shell] => {
            eprintln!("nova: unsupported init shell `{shell}`");
            ExitCode::FAILURE
        }
        [] => {
            eprintln!("nova: init requires a shell name");
            ExitCode::FAILURE
        }
        _ => {
            eprintln!("nova: init accepts only one shell name");
            ExitCode::FAILURE
        }
    }
}

fn run_prompt(args: &[String]) -> ExitCode {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{PROMPT_USAGE}");
        return ExitCode::SUCCESS;
    }

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
        Ok(warnings) => {
            for warning in warnings {
                eprintln!("nova: warning: {warning}");
            }
            println!("nova: config ok");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nova: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "Usage: nova <init|worker|prompt|check>\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version"
    );
}

#[derive(Clone, Debug)]
struct PromptArgs {
    cwd: PathBuf,
    columns: u16,
    exit_status: i32,
    duration_ms: Option<u64>,
    time: Option<String>,
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
                .filter(|columns| *columns > 0)
                .unwrap_or(80),
            exit_status: 0,
            duration_ms: None,
            time: None,
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
                    parsed.columns = parse_columns(required_value(args, index, "--cols")?)?;
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
                "--time" => {
                    parsed.time = Some(required_value(args, index, "--time")?.to_string());
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
                time: self.time,
                columns: self.columns,
                keymap: self.keymap,
                env: nova::state::PromptEnv::from_current_process(),
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
            PromptFormat::Preview => {
                if output.rprompt.is_empty() {
                    format!("{}\n", preview_prompt(&output.prompt))
                } else {
                    format!(
                        "{}\n{}\n",
                        preview_prompt(&output.prompt),
                        preview_prompt(&output.rprompt)
                    )
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
struct WorkerArgs {
    runtime_dir: PathBuf,
    session_token: String,
    parent_pid: Option<u32>,
}

impl WorkerArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut runtime_dir = None;
        let mut session_token = env::var("NOVA_SESSION_TOKEN")
            .ok()
            .filter(|token| !token.is_empty());
        let mut parent_pid = env::var("NOVA_PARENT_PID")
            .ok()
            .filter(|pid| !pid.is_empty())
            .map(|pid| parse_parent_pid(&pid, "NOVA_PARENT_PID"))
            .transpose()?;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--dir" => {
                    runtime_dir = Some(PathBuf::from(required_value(args, index, "--dir")?));
                    index += 2;
                }
                "--session-token" => {
                    session_token =
                        Some(required_value(args, index, "--session-token")?.to_string());
                    index += 2;
                }
                "--parent-pid" => {
                    parent_pid = Some(parse_parent_pid(
                        required_value(args, index, "--parent-pid")?,
                        "--parent-pid",
                    )?);
                    index += 2;
                }
                option => return Err(format!("unknown worker option `{option}`")),
            }
        }

        Ok(Self {
            runtime_dir: runtime_dir.ok_or_else(|| "worker requires --dir".to_string())?,
            session_token: session_token.ok_or_else(|| {
                "worker requires NOVA_SESSION_TOKEN or --session-token".to_string()
            })?,
            parent_pid,
        })
    }

    fn run(self) -> Result<(), String> {
        run_worker_loop(WorkerOptions {
            runtime_dir: self.runtime_dir,
            session_token: self.session_token,
            parent_pid: self.parent_pid,
        })
        .map_err(|error| error.to_string())
    }
}

fn run_worker(args: &[String]) -> ExitCode {
    match WorkerArgs::parse(args).and_then(WorkerArgs::run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nova: {error}");
            ExitCode::FAILURE
        }
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

    fn check(self) -> Result<Vec<ConfigWarning>, String> {
        let config = load_config(self.config_path.as_deref()).map_err(|error| error.to_string())?;
        Ok(config.warnings(known_segment_ids()))
    }
}

#[derive(Clone, Copy, Debug)]
enum PromptFormat {
    Plain,
    Preview,
    Shell,
}

impl PromptFormat {
    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "plain" => Ok(Self::Plain),
            "preview" => Ok(Self::Preview),
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

fn parse_columns(value: &str) -> Result<u16, String> {
    let columns = parse_value(value, "--cols")?;
    if columns == 0 {
        Err("--cols must be greater than 0".to_string())
    } else {
        Ok(columns)
    }
}

fn parse_parent_pid(value: &str, source: &str) -> Result<u32, String> {
    let pid = parse_value(value, source)?;
    if pid == 0 {
        Err(format!("{source} must be greater than 0"))
    } else {
        Ok(pid)
    }
}

fn zsh_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn preview_prompt(input: &str) -> String {
    input.replace("%{", "").replace("%}", "").replace("%%", "%")
}
