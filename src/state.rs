//! Prompt inputs captured by the shell adapter.

use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptState {
    pub cwd: PathBuf,
    pub exit_status: i32,
    pub duration_ms: Option<u64>,
    pub columns: u16,
    pub keymap: Keymap,
    pub env: PromptEnv,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptEnv {
    pub virtual_env: Option<PathBuf>,
    pub in_nix_shell: Option<String>,
    pub nix_shell_name: Option<String>,
    pub nix_shell_level: Option<String>,
}

impl PromptEnv {
    pub fn from_current_process() -> Self {
        Self {
            virtual_env: env_path("VIRTUAL_ENV"),
            in_nix_shell: env_string("IN_NIX_SHELL"),
            nix_shell_name: env_string("name"),
            nix_shell_level: env_string("NIX_SHELL_LEVEL"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keymap {
    #[default]
    Main,
    ViCommand,
}

fn env_string(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_path(name: &str) -> Option<PathBuf> {
    env_string(name).map(PathBuf::from)
}

impl Keymap {
    pub fn parse(input: &str) -> Self {
        match input {
            "vicmd" | "vi-command" => Self::ViCommand,
            _ => Self::Main,
        }
    }
}
