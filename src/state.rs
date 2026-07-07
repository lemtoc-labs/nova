//! Prompt inputs captured by the shell adapter.

use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptState {
    pub cwd: PathBuf,
    pub exit_status: i32,
    pub duration_ms: Option<u64>,
    pub time: Option<String>,
    pub columns: u16,
    pub keymap: Keymap,
    pub env: PromptEnv,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptEnv {
    pub user: Option<String>,
    pub host: Option<String>,
    pub virtual_env: Option<PathBuf>,
    pub in_nix_shell: Option<String>,
    pub nix_shell_name: Option<String>,
    pub nix_shell_level: Option<String>,
    pub home: Option<PathBuf>,
    pub aws: AwsEnv,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AwsEnv {
    pub awsu_profile: Option<String>,
    pub aws_vault: Option<String>,
    pub awsume_profile: Option<String>,
    pub aws_profile: Option<String>,
    pub aws_sso_profile: Option<String>,
    pub aws_region: Option<String>,
    pub aws_default_region: Option<String>,
    pub aws_config_file: Option<PathBuf>,
    pub aws_shared_credentials_file: Option<PathBuf>,
    pub aws_credentials_file: Option<PathBuf>,
    pub aws_access_key_id_present: bool,
    pub aws_secret_access_key_present: bool,
    pub aws_session_token_present: bool,
}

impl PromptEnv {
    pub fn from_current_process() -> Self {
        Self {
            user: env_string("USER"),
            host: env_string("HOST").or_else(|| env_string("HOSTNAME")),
            virtual_env: env_path("VIRTUAL_ENV"),
            in_nix_shell: env_string("IN_NIX_SHELL"),
            nix_shell_name: env_string("name"),
            nix_shell_level: env_string("NIX_SHELL_LEVEL"),
            home: env_path("HOME"),
            aws: AwsEnv::from_current_process(),
        }
    }
}

impl AwsEnv {
    fn from_current_process() -> Self {
        Self {
            awsu_profile: env_string("AWSU_PROFILE"),
            aws_vault: env_string("AWS_VAULT"),
            awsume_profile: env_string("AWSUME_PROFILE"),
            aws_profile: env_string("AWS_PROFILE"),
            aws_sso_profile: env_string("AWS_SSO_PROFILE"),
            aws_region: env_string("AWS_REGION"),
            aws_default_region: env_string("AWS_DEFAULT_REGION"),
            aws_config_file: env_path("AWS_CONFIG_FILE"),
            aws_shared_credentials_file: env_path("AWS_SHARED_CREDENTIALS_FILE"),
            aws_credentials_file: env_path("AWS_CREDENTIALS_FILE"),
            aws_access_key_id_present: env_present("AWS_ACCESS_KEY_ID"),
            aws_secret_access_key_present: env_present("AWS_SECRET_ACCESS_KEY"),
            aws_session_token_present: env_present("AWS_SESSION_TOKEN"),
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

fn env_present(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

impl Keymap {
    pub fn parse(input: &str) -> Self {
        match input {
            "vicmd" | "vi-command" => Self::ViCommand,
            _ => Self::Main,
        }
    }
}
