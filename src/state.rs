//! Prompt inputs captured by the shell adapter.

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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keymap {
    #[default]
    Main,
    ViCommand,
}

impl Keymap {
    pub fn parse(input: &str) -> Self {
        match input {
            "vicmd" | "vi-command" => Self::ViCommand,
            _ => Self::Main,
        }
    }
}
