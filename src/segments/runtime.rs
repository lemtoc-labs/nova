//! Runtime and tool information collectors.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use thiserror::Error;
use wait_timeout::ChildExt;

use crate::cache::CacheKey;
use crate::config::SegmentConfig;
use crate::segments::{SegmentContent, Style, label_with_icon};
use crate::state::PromptEnv;

const RUST_VERSION_SEGMENT_ID: &str = "rust_version";
const RUST_VERSION_ICON: &str = "";
const RUSTC_ARGS: &[&str] = &["--version"];
const RUST_MARKERS: &[&str] = &["Cargo.toml", "rust-toolchain", "rust-toolchain.toml"];
const BUN_VERSION_SEGMENT_ID: &str = "bun_version";
const BUN_VERSION_ICON: &str = "";
const BUN_ARGS: &[&str] = &["--version"];
const BUN_DETECT_FILES: &[&str] = &["bun.lock", "bun.lockb", "bunfig.toml"];
const DENO_VERSION_SEGMENT_ID: &str = "deno_version";
const DENO_VERSION_ICON: &str = "";
const DENO_ARGS: &[&str] = &["-V"];
const DENO_DETECT_FILES: &[&str] = &[
    "deno.json",
    "deno.jsonc",
    "deno.lock",
    "mod.ts",
    "deps.ts",
    "mod.js",
    "deps.js",
];
const PYTHON_VERSION_SEGMENT_ID: &str = "python_version";
const PYTHON_VERSION_ICON: &str = "";
const PYTHON_ARGS: &[&str] = &["--version"];
const PYTHON_COMMANDS: &[&str] = &["python", "python3", "python2"];
const PYTHON_DETECT_EXTENSIONS: &[&str] = &["py", "ipynb"];
const PYTHON_DETECT_FILES: &[&str] = &[
    "requirements.txt",
    ".python-version",
    "pyproject.toml",
    "Pipfile",
    "tox.ini",
    "setup.py",
    "__init__.py",
];
const NIX_SHELL_SEGMENT_ID: &str = "nix_shell";
const NIX_SHELL_ICON: &str = "";
const AWS_SEGMENT_ID: &str = "aws";
const AWS_ICON: &str = "";
const NODE_VERSION_SEGMENT_ID: &str = "node_version";
const NODE_VERSION_ICON: &str = "";
const NODE_ARGS: &[&str] = &["--version"];
const NODE_DETECT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts", "cts"];
const NODE_DETECT_FILES: &[&str] = &["package.json", ".node-version", ".nvmrc"];
const NODE_EXCLUDED_FILES: &[&str] = &[
    "bunfig.toml",
    "bun.lock",
    "bun.lockb",
    "deno.json",
    "deno.jsonc",
    "deno.lock",
];
const NODE_DETECT_FOLDERS: &[&str] = &["node_modules"];
const NODE_EXCLUDED_FOLDERS: &[&str] = &["esy.lock"];

#[derive(Debug, Error)]
pub enum RuntimeCollectError {
    #[error("runtime collection timed out")]
    TimedOut,
    #[error("failed to spawn runtime command: {0}")]
    Spawn(std::io::Error),
    #[error("failed to wait for runtime command: {0}")]
    Wait(std::io::Error),
    #[error("failed to read runtime command output: {0}")]
    ReadOutput(std::io::Error),
    #[error("runtime command output reader panicked")]
    ReaderPanicked,
    #[error("failed to capture runtime command output")]
    MissingStdout,
}

pub fn rust_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    let root = find_rust_project_root(cwd)?;
    Some(CacheKey::new(
        RUST_VERSION_SEGMENT_ID,
        root.to_string_lossy(),
        config_generation,
    ))
}

pub fn node_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    is_node_project_dir(cwd).then(|| {
        CacheKey::new(
            NODE_VERSION_SEGMENT_ID,
            cwd.to_string_lossy(),
            config_generation,
        )
    })
}

pub fn bun_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    is_bun_project_dir(cwd).then(|| {
        CacheKey::new(
            BUN_VERSION_SEGMENT_ID,
            cwd.to_string_lossy(),
            config_generation,
        )
    })
}

pub fn deno_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    is_deno_project_dir(cwd).then(|| {
        CacheKey::new(
            DENO_VERSION_SEGMENT_ID,
            cwd.to_string_lossy(),
            config_generation,
        )
    })
}

pub fn python_cache_key(
    cwd: &Path,
    virtual_env: Option<&Path>,
    config_generation: u64,
) -> Option<CacheKey> {
    is_python_project_dir(cwd, virtual_env).then(|| {
        CacheKey::new(
            PYTHON_VERSION_SEGMENT_ID,
            python_cache_source(cwd, virtual_env),
            config_generation,
        )
    })
}

pub fn find_rust_project_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = cwd;

    loop {
        if RUST_MARKERS
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

pub fn is_node_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: NODE_DETECT_FILES,
            excluded_files: NODE_EXCLUDED_FILES,
            folders: NODE_DETECT_FOLDERS,
            excluded_folders: NODE_EXCLUDED_FOLDERS,
            extensions: NODE_DETECT_EXTENSIONS,
        },
    )
}

pub fn is_bun_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: BUN_DETECT_FILES,
            excluded_files: &[],
            folders: &[],
            excluded_folders: &[],
            extensions: &[],
        },
    )
}

pub fn is_deno_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: DENO_DETECT_FILES,
            excluded_files: &[],
            folders: &[],
            excluded_folders: &[],
            extensions: &[],
        },
    )
}

pub fn is_python_project_dir(cwd: &Path, virtual_env: Option<&Path>) -> bool {
    virtual_env.is_some()
        || current_dir_matches(
            cwd,
            RuntimeDetection {
                files: PYTHON_DETECT_FILES,
                excluded_files: &[],
                folders: &[],
                excluded_folders: &[],
                extensions: PYTHON_DETECT_EXTENSIONS,
            },
        )
}

pub fn collect_rust_version(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_rust_version_with_command(cwd, deadline, Path::new("rustc"))
}

pub fn collect_node_version(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_node_version_with_command(cwd, deadline, Path::new("node"))
}

pub fn collect_bun_version(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_bun_version_with_command(cwd, deadline, Path::new("bun"))
}

pub fn collect_deno_version(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_deno_version_with_command(cwd, deadline, Path::new("deno"))
}

pub fn collect_python_version(
    cwd: &Path,
    virtual_env: Option<&Path>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    let commands = python_command_paths(virtual_env);
    collect_python_version_with_commands(cwd, virtual_env, deadline, &commands)
}

fn collect_rust_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    let Some(root) = find_rust_project_root(cwd) else {
        return Ok(None);
    };
    let timeout = remaining_time(deadline)?;
    let mut child = Command::new(command)
        .args(RUSTC_ARGS)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(RuntimeCollectError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(RuntimeCollectError::MissingStdout)?;
    let stdout_reader = read_stdout(stdout);

    let status = match child
        .wait_timeout(timeout)
        .map_err(RuntimeCollectError::Wait)?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            return Err(RuntimeCollectError::TimedOut);
        }
    };
    let output = join_stdout(stdout_reader)?;

    if !status.success() {
        return Ok(None);
    }

    Ok(parse_rustc_version(&String::from_utf8_lossy(&output)))
}

fn collect_node_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    if !is_node_project_dir(cwd) {
        return Ok(None);
    }

    let timeout = remaining_time(deadline)?;
    let output = collect_command_output(command, NODE_ARGS, cwd, timeout)?;

    Ok(parse_node_version(&String::from_utf8_lossy(&output)))
}

fn collect_bun_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    if !is_bun_project_dir(cwd) {
        return Ok(None);
    }

    let timeout = remaining_time(deadline)?;
    let output = collect_command_output(command, BUN_ARGS, cwd, timeout)?;

    Ok(parse_bun_version(&String::from_utf8_lossy(&output)))
}

fn collect_deno_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    if !is_deno_project_dir(cwd) {
        return Ok(None);
    }

    let timeout = remaining_time(deadline)?;
    let output = collect_command_output(command, DENO_ARGS, cwd, timeout)?;

    Ok(parse_deno_version(&String::from_utf8_lossy(&output)))
}

fn collect_python_version_with_commands(
    cwd: &Path,
    virtual_env: Option<&Path>,
    deadline: Instant,
    commands: &[PathBuf],
) -> Result<Option<String>, RuntimeCollectError> {
    if !is_python_project_dir(cwd, virtual_env) {
        return Ok(None);
    }

    for command in commands {
        let timeout = remaining_time(deadline)?;
        match collect_command_output(command, PYTHON_ARGS, cwd, timeout) {
            Ok(output) => {
                if let Some(version) = parse_python_version(&String::from_utf8_lossy(&output)) {
                    return Ok(Some(version));
                }
            }
            Err(RuntimeCollectError::Spawn(error))
                if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

fn collect_command_output(
    command: &Path,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<Vec<u8>, RuntimeCollectError> {
    let mut child = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(RuntimeCollectError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(RuntimeCollectError::MissingStdout)?;
    let stdout_reader = read_stdout(stdout);

    let status = match child
        .wait_timeout(timeout)
        .map_err(RuntimeCollectError::Wait)?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            return Err(RuntimeCollectError::TimedOut);
        }
    };
    let output = join_stdout(stdout_reader)?;

    if !status.success() {
        return Ok(Vec::new());
    }

    Ok(output)
}

pub fn parse_rustc_version(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("rustc"), Some(version)) if !version.is_empty() => Some(version.to_string()),
        _ => None,
    }
}

pub fn parse_node_version(output: &str) -> Option<String> {
    let version = output
        .split_whitespace()
        .next()?
        .trim_start_matches('v')
        .trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub fn parse_bun_version(output: &str) -> Option<String> {
    let version = output.trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub fn parse_deno_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(ToString::to_string)
}

pub fn parse_python_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(ToString::to_string)
}

pub fn render_rust_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        RUST_VERSION_SEGMENT_ID,
        label_with_icon(version, config, RUST_VERSION_ICON),
        rust_style(config),
    ))
}

pub fn render_node_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        NODE_VERSION_SEGMENT_ID,
        label_with_icon(version, config, NODE_VERSION_ICON),
        node_style(config),
    ))
}

pub fn render_bun_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        BUN_VERSION_SEGMENT_ID,
        label_with_icon(version, config, BUN_VERSION_ICON),
        bun_style(config),
    ))
}

pub fn render_deno_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        DENO_VERSION_SEGMENT_ID,
        label_with_icon(version, config, DENO_VERSION_ICON),
        deno_style(config),
    ))
}

pub fn render_python_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        PYTHON_VERSION_SEGMENT_ID,
        label_with_icon(version, config, PYTHON_VERSION_ICON),
        python_style(config),
    ))
}

pub fn render_nix_shell(env: &PromptEnv, config: &SegmentConfig) -> Option<SegmentContent> {
    let state = match env.in_nix_shell.as_deref()? {
        "pure" => "pure",
        "impure" => "impure",
        _ => return None,
    };
    let label = match env.nix_shell_name.as_deref() {
        Some(name) => format!("{state} ({name})"),
        None => state.to_string(),
    };

    Some(SegmentContent::new(
        NIX_SHELL_SEGMENT_ID,
        label_with_icon(&label, config, NIX_SHELL_ICON),
        nix_shell_style(config),
    ))
}

pub fn render_aws(env: &PromptEnv, config: &SegmentConfig) -> Option<SegmentContent> {
    let force_display = config.force_display.unwrap_or(true);
    let context = resolve_aws_context(env, force_display)?;
    let text = match config.format.as_deref() {
        Some(format) => render_aws_format(format, &context, config),
        None => label_with_icon(&context.label(), config, AWS_ICON),
    };
    if text.is_empty() {
        return None;
    }

    Some(SegmentContent::new(AWS_SEGMENT_ID, text, aws_style(config)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AwsContext {
    profile: Option<String>,
    region: Option<String>,
}

impl AwsContext {
    fn label(&self) -> String {
        match (self.profile.as_deref(), self.region.as_deref()) {
            (Some(profile), Some(region)) => format!("{profile} ({region})"),
            (Some(profile), None) => profile.to_string(),
            (None, Some(region)) => format!("({region})"),
            (None, None) => String::new(),
        }
    }
}

struct AwsFormatVariables<'a> {
    symbol: String,
    profile: Option<&'a str>,
    region: Option<&'a str>,
}

fn render_aws_format(format: &str, context: &AwsContext, config: &SegmentConfig) -> String {
    let variables = AwsFormatVariables {
        symbol: aws_symbol(config),
        profile: context.profile.as_deref(),
        region: context.region.as_deref(),
    };
    render_aws_format_template(format, &variables)
}

fn render_aws_format_template(format: &str, variables: &AwsFormatVariables<'_>) -> String {
    let chars = format.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut plain = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '[' {
            if let Some(end) = closing_optional_group(&chars, index + 1) {
                output.push_str(&render_aws_format_part(&plain, variables).text);
                plain.clear();
                let inner = chars[index + 1..end].iter().collect::<String>();
                let rendered = render_aws_format_part(&inner, variables);
                if rendered.has_value {
                    output.push_str(&rendered.text);
                }
                index = end + 1;
                continue;
            }
        }

        plain.push(chars[index]);
        index += 1;
    }

    output.push_str(&render_aws_format_part(&plain, variables).text);
    output
}

#[derive(Debug, PartialEq, Eq)]
struct RenderedAwsFormatPart {
    text: String,
    has_value: bool,
}

fn render_aws_format_part(
    input: &str,
    variables: &AwsFormatVariables<'_>,
) -> RenderedAwsFormatPart {
    let chars = input.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut has_value = false;
    let mut index = 0;

    while index < chars.len() {
        if chars[index] != '$' {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < chars.len() && is_format_variable_char(chars[end]) {
            end += 1;
        }

        if start == end {
            output.push('$');
            index += 1;
            continue;
        }

        let name = chars[start..end].iter().collect::<String>();
        match aws_format_value(&name, variables) {
            Some(value) => {
                has_value |= !value.is_empty();
                output.push_str(&value);
            }
            None => {
                output.push('$');
                output.push_str(&name);
            }
        }
        index = end;
    }

    RenderedAwsFormatPart {
        text: output,
        has_value,
    }
}

fn closing_optional_group(chars: &[char], start: usize) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, character)| (*character == ']').then_some(index))
}

fn is_format_variable_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn aws_format_value(name: &str, variables: &AwsFormatVariables<'_>) -> Option<String> {
    match name {
        "symbol" => Some(variables.symbol.clone()),
        "profile" => Some(variables.profile.unwrap_or_default().to_string()),
        "region" => Some(variables.region.unwrap_or_default().to_string()),
        "duration" => Some(String::new()),
        _ => None,
    }
}

fn aws_symbol(config: &SegmentConfig) -> String {
    match config.icon.as_deref() {
        Some("") => String::new(),
        Some(icon) => format!("{icon} "),
        None => format!("{AWS_ICON} "),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IniFile {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniFile {
    fn section(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.sections.get(name)
    }
}

fn resolve_aws_context(env: &PromptEnv, force_display: bool) -> Option<AwsContext> {
    let config_file = aws_config_file_path(env).and_then(read_ini_file);
    let credentials_file = aws_credentials_file_path(env).and_then(read_ini_file);
    let profile = aws_profile(env);
    let region = aws_region(env, profile.as_ref(), config_file.as_ref());

    if profile.is_none() && region.is_none() {
        return None;
    }

    if !force_display
        && !has_credential_process_or_sso(
            config_file.as_ref(),
            credentials_file.as_ref(),
            profile.as_ref(),
        )
        && !has_source_profile(
            config_file.as_ref(),
            credentials_file.as_ref(),
            profile.as_ref(),
        )
        && !has_defined_credentials(env, credentials_file.as_ref(), profile.as_ref())
    {
        return None;
    }

    Some(AwsContext { profile, region })
}

fn aws_profile(env: &PromptEnv) -> Option<String> {
    [
        env.aws.awsu_profile.as_ref(),
        env.aws.aws_vault.as_ref(),
        env.aws.awsume_profile.as_ref(),
        env.aws.aws_profile.as_ref(),
        env.aws.aws_sso_profile.as_ref(),
    ]
    .into_iter()
    .flatten()
    .next()
    .cloned()
}

fn aws_region(
    env: &PromptEnv,
    profile: Option<&String>,
    config_file: Option<&IniFile>,
) -> Option<String> {
    env.aws
        .aws_region
        .clone()
        .or_else(|| env.aws.aws_default_region.clone())
        .or_else(|| {
            aws_config_section(config_file, profile)?
                .get("region")
                .cloned()
        })
}

fn has_credential_process_or_sso(
    config_file: Option<&IniFile>,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    let config_has = aws_config_section(config_file, profile).is_some_and(|section| {
        section.contains_key("credential_process")
            || section.contains_key("sso_session")
            || section.contains_key("sso_start_url")
    });
    if config_has {
        return true;
    }

    aws_credentials_section(credentials_file, profile).is_some_and(|section| {
        section.contains_key("credential_process") || section.contains_key("sso_start_url")
    })
}

fn has_source_profile(
    config_file: Option<&IniFile>,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    let Some(source_profile) =
        aws_config_section(config_file, profile).and_then(|section| section.get("source_profile"))
    else {
        return false;
    };

    has_credential_process_or_sso(config_file, credentials_file, Some(source_profile))
        || has_defined_credentials_for_profile(credentials_file, Some(source_profile))
}

fn has_defined_credentials(
    env: &PromptEnv,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    env.aws.aws_access_key_id_present
        || env.aws.aws_secret_access_key_present
        || env.aws.aws_session_token_present
        || has_defined_credentials_for_profile(credentials_file, profile)
}

fn has_defined_credentials_for_profile(
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    aws_credentials_section(credentials_file, profile)
        .is_some_and(|section| section.contains_key("aws_access_key_id"))
}

fn aws_config_section<'a>(
    config_file: Option<&'a IniFile>,
    profile: Option<&String>,
) -> Option<&'a BTreeMap<String, String>> {
    let section_name = match profile {
        Some(profile) => format!("profile {profile}"),
        None => "default".to_string(),
    };
    config_file?.section(&section_name)
}

fn aws_credentials_section<'a>(
    credentials_file: Option<&'a IniFile>,
    profile: Option<&String>,
) -> Option<&'a BTreeMap<String, String>> {
    let section_name = profile.map_or("default", String::as_str);
    credentials_file?.section(section_name)
}

fn aws_config_file_path(env: &PromptEnv) -> Option<PathBuf> {
    env.aws
        .aws_config_file
        .clone()
        .or_else(|| env.home.as_ref().map(|home| home.join(".aws/config")))
}

fn aws_credentials_file_path(env: &PromptEnv) -> Option<PathBuf> {
    env.aws
        .aws_shared_credentials_file
        .clone()
        .or_else(|| env.aws.aws_credentials_file.clone())
        .or_else(|| env.home.as_ref().map(|home| home.join(".aws/credentials")))
}

fn read_ini_file(path: PathBuf) -> Option<IniFile> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| parse_ini(&contents))
}

fn parse_ini(input: &str) -> IniFile {
    let mut file = IniFile::default();
    let mut current_section = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
            .map(str::trim)
            .filter(|section| !section.is_empty())
        {
            current_section = Some(section.to_string());
            file.sections.entry(section.to_string()).or_default();
            continue;
        }

        let Some(section) = current_section.as_ref() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }

        file.sections
            .entry(section.clone())
            .or_default()
            .insert(key.to_string(), value.trim().to_string());
    }

    file
}

#[derive(Clone, Copy)]
struct RuntimeDetection<'a> {
    files: &'a [&'a str],
    excluded_files: &'a [&'a str],
    folders: &'a [&'a str],
    excluded_folders: &'a [&'a str],
    extensions: &'a [&'a str],
}

fn current_dir_matches(cwd: &Path, detection: RuntimeDetection<'_>) -> bool {
    let Ok(entries) = fs::read_dir(cwd) else {
        return false;
    };

    let mut has_positive_match = false;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let is_dir = entry.file_type().is_ok_and(|file_type| file_type.is_dir());

        if is_dir {
            if detection.excluded_folders.contains(&file_name) {
                return false;
            }
            if detection.folders.contains(&file_name) {
                has_positive_match = true;
            }
        } else {
            if detection.excluded_files.contains(&file_name) {
                return false;
            }
            if detection.files.contains(&file_name)
                || file_has_any_extension(file_name, detection.extensions)
            {
                has_positive_match = true;
            }
        }
    }

    has_positive_match
}

fn file_has_any_extension(file_name: &str, extensions: &[&str]) -> bool {
    if extensions.is_empty() || file_name.starts_with('.') {
        return false;
    }

    let path = Path::new(file_name);
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension))
        || file_name
            .split_once('.')
            .is_some_and(|(_name, extension)| extensions.contains(&extension))
}

fn python_cache_source(cwd: &Path, virtual_env: Option<&Path>) -> String {
    match virtual_env {
        Some(virtual_env) => format!(
            "{}|{}",
            cwd.to_string_lossy(),
            virtual_env.to_string_lossy()
        ),
        None => cwd.to_string_lossy().into_owned(),
    }
}

fn python_command_paths(virtual_env: Option<&Path>) -> Vec<PathBuf> {
    let venv_python = virtual_env.map(|virtual_env| virtual_env.join("bin").join("python"));
    venv_python
        .into_iter()
        .chain(PYTHON_COMMANDS.iter().map(PathBuf::from))
        .collect()
}

fn remaining_time(deadline: Instant) -> Result<Duration, RuntimeCollectError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(RuntimeCollectError::TimedOut)?;

    if remaining.is_zero() {
        Err(RuntimeCollectError::TimedOut)
    } else {
        Ok(remaining)
    }
}

fn read_stdout(mut stdout: std::process::ChildStdout) -> JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_stdout(
    stdout_reader: JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, RuntimeCollectError> {
    stdout_reader
        .join()
        .map_err(|_panic| RuntimeCollectError::ReaderPanicked)?
        .map_err(RuntimeCollectError::ReadOutput)
}

fn rust_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("cyan".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn node_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("green".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn bun_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("red".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn deno_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("green".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn python_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("yellow".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn nix_shell_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("blue".to_string()),
            bg: None,
            bold: true,
        }
    }
}

fn aws_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("yellow".to_string()),
            bg: None,
            bold: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;
    use std::time::Duration;

    use crate::state::AwsEnv;

    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn finds_rust_project_root_from_markers() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let nested = tempdir.path().join("crates").join("nova");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");

        assert_eq!(
            find_rust_project_root(&nested),
            Some(tempdir.path().to_path_buf())
        );
    }

    #[test]
    fn cache_key_is_none_outside_rust_projects() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");

        assert_eq!(rust_cache_key(tempdir.path(), 1), None);
    }

    #[test]
    fn detects_node_projects_from_starship_default_conditions() {
        for marker in ["package.json", ".node-version", ".nvmrc"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_node_project_dir(tempdir.path()),
                "{marker} should trigger node detection"
            );
        }

        for file in [
            "index.js",
            "index.mjs",
            "index.cjs",
            "index.ts",
            "index.mts",
            "index.cts",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(file), "").expect("source file should be written");

            assert!(
                is_node_project_dir(tempdir.path()),
                "{file} should trigger node detection"
            );
        }

        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir(tempdir.path().join("node_modules"))
            .expect("node_modules should be created");

        assert!(is_node_project_dir(tempdir.path()));
    }

    #[test]
    fn excludes_node_projects_with_starship_default_negative_conditions() {
        for excluded in [
            "bunfig.toml",
            "bun.lock",
            "bun.lockb",
            "deno.json",
            "deno.jsonc",
            "deno.lock",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(excluded), "").expect("excluded file should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{excluded} should suppress node detection"
            );
        }

        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        fs::create_dir(tempdir.path().join("esy.lock")).expect("esy.lock should be created");

        assert!(!is_node_project_dir(tempdir.path()));
    }

    #[test]
    fn detects_bun_projects_from_starship_default_conditions() {
        for marker in ["bun.lock", "bun.lockb", "bunfig.toml"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_bun_project_dir(tempdir.path()),
                "{marker} should trigger bun detection"
            );
        }
    }

    #[test]
    fn bun_projects_suppress_node_detection() {
        for marker in ["bun.lock", "bun.lockb", "bunfig.toml"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(marker), "").expect("bun marker should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{marker} should suppress node detection"
            );
            assert!(
                is_bun_project_dir(tempdir.path()),
                "{marker} should trigger bun detection"
            );
        }
    }

    #[test]
    fn detects_deno_projects_from_starship_default_conditions() {
        for marker in [
            "deno.json",
            "deno.jsonc",
            "deno.lock",
            "mod.ts",
            "deps.ts",
            "mod.js",
            "deps.js",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_deno_project_dir(tempdir.path()),
                "{marker} should trigger deno detection"
            );
        }
    }

    #[test]
    fn deno_projects_suppress_node_detection() {
        for marker in ["deno.json", "deno.jsonc", "deno.lock"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(marker), "").expect("deno marker should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{marker} should suppress node detection"
            );
            assert!(
                is_deno_project_dir(tempdir.path()),
                "{marker} should trigger deno detection"
            );
        }
    }

    #[test]
    fn detects_python_projects_from_starship_default_conditions() {
        for marker in [
            "requirements.txt",
            ".python-version",
            "pyproject.toml",
            "Pipfile",
            "tox.ini",
            "setup.py",
            "__init__.py",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_python_project_dir(tempdir.path(), None),
                "{marker} should trigger python detection"
            );
        }

        for file in ["main.py", "notebook.ipynb"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(file), "").expect("source file should be written");

            assert!(
                is_python_project_dir(tempdir.path(), None),
                "{file} should trigger python detection"
            );
        }
    }

    #[test]
    fn detects_python_projects_from_virtual_env() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let virtual_env = tempdir.path().join(".venv");

        assert!(is_python_project_dir(tempdir.path(), Some(&virtual_env)));
    }

    #[test]
    fn parses_rustc_version_output() {
        assert_eq!(
            parse_rustc_version("rustc 1.96.1 (abc 2026-01-01)\n"),
            Some("1.96.1".to_string())
        );
        assert_eq!(parse_rustc_version("not rustc\n"), None);
    }

    #[test]
    fn parses_node_version_output() {
        assert_eq!(
            parse_node_version("v22.17.0\n"),
            Some("22.17.0".to_string())
        );
        assert_eq!(parse_node_version("22.17.0\n"), Some("22.17.0".to_string()));
        assert_eq!(parse_node_version("\n"), None);
    }

    #[test]
    fn parses_bun_version_output() {
        assert_eq!(parse_bun_version("1.2.18\n"), Some("1.2.18".to_string()));
        assert_eq!(parse_bun_version("\n"), None);
    }

    #[test]
    fn parses_deno_version_output() {
        assert_eq!(
            parse_deno_version("deno 2.3.6\n"),
            Some("2.3.6".to_string())
        );
        assert_eq!(parse_deno_version("not-enough\n"), None);
    }

    #[test]
    fn parses_python_version_output() {
        assert_eq!(
            parse_python_version("Python 3.12.4\n"),
            Some("3.12.4".to_string())
        );
        assert_eq!(
            parse_python_version("Python 3.12.4 :: Anaconda, Inc.\n"),
            Some("3.12.4".to_string())
        );
        assert_eq!(parse_python_version("not-enough\n"), None);
    }

    #[test]
    fn renders_rust_version_segment() {
        let segment = render_rust_version("1.96.1", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "rust_version");
        assert_eq!(segment.text, " 1.96.1");
        assert_eq!(segment.style.fg.as_deref(), Some("cyan"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_rust_version_with_configured_icon() {
        let segment = render_rust_version(
            "1.96.1",
            &SegmentConfig {
                icon: Some("rust".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "rust 1.96.1");
    }

    #[test]
    fn renders_rust_version_without_icon_when_configured_empty() {
        let segment = render_rust_version(
            "1.96.1",
            &SegmentConfig {
                icon: Some(String::new()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "1.96.1");
    }

    #[test]
    fn renders_node_version_segment() {
        let segment = render_node_version("22.17.0", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "node_version");
        assert_eq!(segment.text, " 22.17.0");
        assert_eq!(segment.style.fg.as_deref(), Some("green"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_node_version_with_configured_icon() {
        let segment = render_node_version(
            "22.17.0",
            &SegmentConfig {
                icon: Some("node".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "node 22.17.0");
    }

    #[test]
    fn renders_bun_version_segment() {
        let segment =
            render_bun_version("1.2.18", &SegmentConfig::default()).expect("version should render");

        assert_eq!(segment.id, "bun_version");
        assert_eq!(segment.text, " 1.2.18");
        assert_eq!(segment.style.fg.as_deref(), Some("red"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_bun_version_with_configured_icon() {
        let segment = render_bun_version(
            "1.2.18",
            &SegmentConfig {
                icon: Some("bun".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "bun 1.2.18");
    }

    #[test]
    fn renders_deno_version_segment() {
        let segment =
            render_deno_version("2.3.6", &SegmentConfig::default()).expect("version should render");

        assert_eq!(segment.id, "deno_version");
        assert_eq!(segment.text, " 2.3.6");
        assert_eq!(segment.style.fg.as_deref(), Some("green"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_deno_version_with_configured_icon() {
        let segment = render_deno_version(
            "2.3.6",
            &SegmentConfig {
                icon: Some("deno".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "deno 2.3.6");
    }

    #[test]
    fn renders_python_version_segment() {
        let segment = render_python_version("3.12.4", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "python_version");
        assert_eq!(segment.text, " 3.12.4");
        assert_eq!(segment.style.fg.as_deref(), Some("yellow"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_python_version_with_configured_icon() {
        let segment = render_python_version(
            "3.12.4",
            &SegmentConfig {
                icon: Some("py".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "py 3.12.4");
    }

    #[test]
    fn renders_nix_shell_segment_for_pure_shell() {
        let segment = render_nix_shell(
            &PromptEnv {
                in_nix_shell: Some("pure".to_string()),
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("nix shell should render");

        assert_eq!(segment.id, "nix_shell");
        assert_eq!(segment.text, " pure");
        assert_eq!(segment.style.fg.as_deref(), Some("blue"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_nix_shell_segment_with_name() {
        let segment = render_nix_shell(
            &PromptEnv {
                in_nix_shell: Some("impure".to_string()),
                nix_shell_name: Some("starship".to_string()),
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("nix shell should render");

        assert_eq!(segment.text, " impure (starship)");
    }

    #[test]
    fn omits_nix_shell_segment_for_invalid_state() {
        assert_eq!(
            render_nix_shell(
                &PromptEnv {
                    in_nix_shell: Some("unknown".to_string()),
                    ..PromptEnv::default()
                },
                &SegmentConfig::default(),
            ),
            None
        );
    }

    #[test]
    fn renders_aws_region_with_env_credentials() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_region: Some("ap-northeast-1".to_string()),
                    aws_access_key_id_present: true,
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.id, "aws");
        assert_eq!(segment.text, " (ap-northeast-1)");
        assert_eq!(segment.style.fg.as_deref(), Some("yellow"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_aws_without_credentials_by_default() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_region: Some("ap-northeast-1".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (ap-northeast-1)");
    }

    #[test]
    fn renders_aws_with_configured_format_hiding_region() {
        let config = SegmentConfig {
            format: Some("$symbol$profile".to_string()),
            ..SegmentConfig::default()
        };
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_region: Some("ap-northeast-1".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &config,
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts");
    }

    #[test]
    fn renders_aws_optional_format_groups_only_when_variables_are_present() {
        let config = SegmentConfig {
            format: Some("$symbol$profile[ ($region)]".to_string()),
            ..SegmentConfig::default()
        };
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &config,
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts");
    }

    #[test]
    fn omits_aws_when_configured_format_renders_empty() {
        let config = SegmentConfig {
            icon: Some(String::new()),
            format: Some("$symbol".to_string()),
            ..SegmentConfig::default()
        };

        assert_eq!(
            render_aws(
                &PromptEnv {
                    aws: AwsEnv {
                        aws_profile: Some("astronauts".to_string()),
                        ..AwsEnv::default()
                    },
                    ..PromptEnv::default()
                },
                &config,
            ),
            None
        );
    }

    #[test]
    fn omits_aws_without_credentials_when_force_display_is_false() {
        let config = SegmentConfig {
            force_display: Some(false),
            ..SegmentConfig::default()
        };

        assert_eq!(
            render_aws(
                &PromptEnv {
                    aws: AwsEnv {
                        aws_profile: Some("astronauts".to_string()),
                        aws_region: Some("ap-northeast-1".to_string()),
                        ..AwsEnv::default()
                    },
                    ..PromptEnv::default()
                },
                &config,
            ),
            None
        );
    }

    #[test]
    fn resolves_aws_profile_using_starship_env_precedence() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    awsu_profile: Some("awsu-profile".to_string()),
                    aws_vault: Some("vault-profile".to_string()),
                    awsume_profile: Some("awsume-profile".to_string()),
                    aws_profile: Some("plain-profile".to_string()),
                    aws_sso_profile: Some("sso-profile".to_string()),
                    aws_access_key_id_present: true,
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " awsu-profile");
    }

    #[test]
    fn reads_aws_profile_region_and_credential_process_from_config() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [default]
            region = us-east-1

            [profile astronauts]
            region = ap-northeast-1
            credential_process = /opt/bin/awscreds-retriever
            "#,
        )
        .expect("config should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (ap-northeast-1)");
    }

    #[test]
    fn accepts_aws_sso_config() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [profile astronauts]
            region = us-east-2
            sso_start_url = https://example.com/start
            "#,
        )
        .expect("config should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (us-east-2)");
    }

    #[test]
    fn reads_aws_default_region_and_default_credentials_from_files() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(&config_path, "[default]\nregion = us-east-1\n")
            .expect("config should be written");
        let credentials_path = tempdir.path().join("credentials");
        fs::write(&credentials_path, "[default]\naws_access_key_id = dummy\n")
            .expect("credentials should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_config_file: Some(config_path),
                    aws_shared_credentials_file: Some(credentials_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " (us-east-1)");
    }

    #[test]
    fn accepts_aws_source_profile_credentials() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [profile astronauts]
            region = us-west-2
            source_profile = base
            "#,
        )
        .expect("config should be written");
        let credentials_path = tempdir.path().join("credentials");
        fs::write(&credentials_path, "[base]\naws_access_key_id = dummy\n")
            .expect("credentials should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    aws_shared_credentials_file: Some(credentials_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (us-west-2)");
    }

    #[test]
    #[cfg(unix)]
    fn collects_rust_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");
        let rustc = write_script(
            tempdir.path(),
            "rustc",
            "printf 'rustc 1.96.1 (abc date)\\n'\n",
        );

        let collected = collect_rust_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &rustc,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("1.96.1".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_rust_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");
        let rustc = write_script(tempdir.path(), "slow-rustc", "sleep 2\n");

        let result = collect_rust_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &rustc,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }

    #[test]
    #[cfg(unix)]
    fn collects_node_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        let node = write_script(tempdir.path(), "node", "printf 'v22.17.0\\n'\n");

        let collected = collect_node_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &node,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("22.17.0".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_node_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        let node = write_script(tempdir.path(), "slow-node", "sleep 2\n");

        let result = collect_node_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &node,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }

    #[test]
    #[cfg(unix)]
    fn collects_bun_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("bun.lock"), "").expect("marker should be written");
        let bun = write_script(tempdir.path(), "bun", "printf '1.2.18\\n'\n");

        let collected = collect_bun_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &bun,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("1.2.18".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_bun_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("bun.lock"), "").expect("marker should be written");
        let bun = write_script(tempdir.path(), "slow-bun", "sleep 2\n");

        let result = collect_bun_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &bun,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }

    #[test]
    #[cfg(unix)]
    fn collects_deno_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("deno.json"), "{}").expect("marker should be written");
        let deno = write_script(tempdir.path(), "deno", "printf 'deno 2.3.6\\n'\n");

        let collected = collect_deno_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &deno,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("2.3.6".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_deno_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("deno.json"), "{}").expect("marker should be written");
        let deno = write_script(tempdir.path(), "slow-deno", "sleep 2\n");

        let result = collect_deno_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &deno,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }

    #[test]
    #[cfg(unix)]
    fn collects_python_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python = write_script(tempdir.path(), "python", "printf 'Python 3.12.4\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_secs(5),
            &[python],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn collects_python_version_from_virtual_env_command_first() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let virtual_env = tempdir.path().join(".venv");
        let bin_dir = virtual_env.join("bin");
        fs::create_dir_all(&bin_dir).expect("venv bin should be created");
        let venv_python = write_script(&bin_dir, "python", "printf 'Python 3.12.4\\n'\n");
        let fallback_python = write_script(tempdir.path(), "python", "printf 'Python 3.11.9\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            Some(&virtual_env),
            Instant::now() + Duration::from_secs(5),
            &[venv_python, fallback_python],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn falls_back_to_later_python_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python3 = write_script(tempdir.path(), "python3", "printf 'Python 3.12.4\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_secs(5),
            &[tempdir.path().join("missing-python"), python3],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_python_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python = write_script(tempdir.path(), "slow-python", "sleep 2\n");

        let result = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_millis(50),
            &[python],
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }

    #[cfg(unix)]
    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        {
            let mut file = fs::File::create(&path).expect("script should be created");
            write!(file, "#!/bin/sh\n{body}").expect("script should be written");
            file.sync_all().expect("script should be synced");
        }
        let mut permissions = fs::metadata(&path)
            .expect("script metadata should be read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("script should be executable");
        let file = fs::File::open(&path).expect("script should be opened for sync");
        file.sync_all()
            .expect("script should be synced after chmod");
        path
    }
}
