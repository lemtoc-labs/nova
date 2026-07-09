//! zsh adapter script embedding.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::worker::protocol;

const INIT_ZSH: &str = include_str!("shell/init.zsh");
const SESSION_TOKEN_BYTES: usize = 16;

pub fn render_init_script(binary_path: &Path) -> String {
    render_init_script_with_session_token_initializer(binary_path, &session_token_initializer())
}

fn render_init_script_with_session_token_initializer(
    binary_path: &Path,
    session_token_initializer: &str,
) -> String {
    INIT_ZSH
        .replace(
            "@NOVA_BIN@",
            &zsh_single_quote(&binary_path.to_string_lossy()),
        )
        .replace("@NOVA_PROTOCOL_VERSION@", protocol::VERSION)
        .replace("@NOVA_SESSION_TOKEN@", session_token_initializer)
}

fn session_token_initializer() -> String {
    match os_random_session_token() {
        Some(token) => zsh_single_quote(&token),
        None => zsh_random_session_token_expression(),
    }
}

fn os_random_session_token() -> Option<String> {
    let mut bytes = [0_u8; SESSION_TOKEN_BYTES];
    let mut random = File::open("/dev/urandom").ok()?;
    random.read_exact(&mut bytes).ok()?;
    Some(hex_encode(&bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn zsh_random_session_token_expression() -> String {
    "\"${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}-$$\"".to_string()
}

#[cfg(test)]
fn render_init_script_with_session_token(binary_path: &Path, session_token: &str) -> String {
    render_init_script_with_session_token_initializer(binary_path, &zsh_single_quote(session_token))
}

fn zsh_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_binary_path_as_a_single_quoted_zsh_string() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova bin"), "token");

        assert!(script.contains("typeset -g _nova_bin='/tmp/nova bin'"));
    }

    #[test]
    fn embeds_the_protocol_version() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("typeset -g _nova_protocol_version=7"));
        assert!(script.contains("\"${fields[2]}\" == \"$_nova_protocol_version\""));
        assert!(!script.contains("@NOVA_PROTOCOL_VERSION@"));
    }

    #[test]
    fn embeds_the_session_token_as_a_single_quoted_zsh_string() {
        let script =
            render_init_script_with_session_token(Path::new("/tmp/nova"), "token'with quote");

        assert!(script.contains("typeset -g _nova_session_token='token'\\''with quote'"));
        assert!(!script.contains("@NOVA_SESSION_TOKEN@"));
    }

    #[test]
    fn reads_initial_wait_budget_from_handshake() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("typeset -g _nova_wait_cs=0"));
        assert!(script.contains("_nova_wait_cs=$(( (${fields[4]:-0} + 9) / 10 ))"));
        assert!(script.contains("[[ \"$_nova_reply_status\" == partial ]]"));
    }

    #[test]
    fn escapes_single_quotes_in_binary_paths() {
        let script = render_init_script_with_session_token(Path::new("/tmp/no'va"), "token");

        assert!(script.contains("typeset -g _nova_bin='/tmp/no'\\''va'"));
    }

    #[test]
    fn registers_zle_line_init_for_pending_updates() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("autoload -Uz add-zsh-hook add-zle-hook-widget"));
        assert!(script.contains("add-zle-hook-widget line-init _nova_zle_line_init"));
    }

    #[test]
    fn registers_zle_keymap_select_for_vi_prompt_char_updates() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("typeset -g _nova_last_exit_status=0"));
        assert!(script.contains("typeset -g _nova_last_duration_ms="));
        assert!(script.contains("_nova_last_exit_status=$exit_status"));
        assert!(script.contains("_nova_last_duration_ms=$duration_ms"));
        assert!(script.contains(
            "_nova_send_request \"$_nova_last_exit_status\" \"$_nova_last_duration_ms\""
        ));
        assert!(script.contains("add-zle-hook-widget keymap-select _nova_zle_keymap_select"));
    }

    #[test]
    fn eagerly_spawns_worker_once_for_interactive_shells() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("_nova_worker_alive && return 0"));
        assert!(script.contains("if ! _nova_worker_alive; then"));
        assert!(script.contains(
            "if [[ -o interactive ]]; then\n  _nova_spawn_worker || true\nfi\n\nadd-zsh-hook"
        ));
    }

    #[test]
    fn hardens_runtime_directory_creation() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("command mkdir -m 700 -- \"$_nova_runtime_dir\""));
        assert!(script.contains("[[ -d \"$_nova_runtime_dir\" && -O \"$_nova_runtime_dir\" ]]"));
        assert!(
            script.contains("command mkfifo -m 600 -- \"$_nova_req_fifo\" \"$_nova_resp_fifo\"")
        );
        assert!(!script.contains("command od -An -N16 -tx1 /dev/urandom"));
        assert!(!script.contains("command tr -d"));
        assert!(!script.contains("command chmod 700"));
        assert!(!script.contains("umask 077"));
    }

    #[test]
    fn passes_worker_session_details_through_environment() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("NOVA_SESSION_TOKEN=\"$_nova_session_token\" NOVA_PARENT_PID=$$"));
        assert!(script.contains("\"$_nova_bin\" worker --dir \"$_nova_runtime_dir\""));
        assert!(!script.contains("--session-token \"$_nova_session_token\""));
    }

    #[test]
    fn sends_virtual_env_in_render_requests() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains(
            "${KEYMAP:-main}${_nova_nul}${USER:-}${_nova_nul}${prompt_host}${_nova_nul}${prompt_time}${_nova_nul}${VIRTUAL_ENV:-}"
        ));
    }

    #[test]
    fn sends_nix_shell_env_in_render_requests() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(
            script
                .contains("${IN_NIX_SHELL:-}${_nova_nul}${name:-}${_nova_nul}${NIX_SHELL_LEVEL:-}")
        );
    }

    #[test]
    fn sends_aws_env_in_render_requests_without_secret_values() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(
            script.contains(
                "${HOME:-}${_nova_nul}${AWSU_PROFILE:-}${_nova_nul}${AWS_VAULT:-}${_nova_nul}${AWSUME_PROFILE:-}${_nova_nul}${AWS_PROFILE:-}${_nova_nul}${AWS_SSO_PROFILE:-}${_nova_nul}${AWS_REGION:-}${_nova_nul}${AWS_DEFAULT_REGION:-}${_nova_nul}${AWS_CONFIG_FILE:-}${_nova_nul}${AWS_SHARED_CREDENTIALS_FILE:-}${_nova_nul}${AWS_CREDENTIALS_FILE:-}${_nova_nul}${AWS_ACCESS_KEY_ID:+1}${_nova_nul}${AWS_SECRET_ACCESS_KEY:+1}${_nova_nul}${AWS_SESSION_TOKEN:+1}"
            )
        );
    }

    #[test]
    fn sends_path_in_render_requests() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("${AWS_SESSION_TOKEN:+1}${_nova_nul}${PATH:-}${_nova_rs}"));
    }

    #[test]
    fn checks_request_write_byte_count() {
        let script = render_init_script_with_session_token(Path::new("/tmp/nova"), "token");

        assert!(script.contains("local -i wrote=0 frame_len=0"));
        assert!(script.contains("setopt localoptions no_multibyte; frame_len=${#1}"));
        assert!(script.contains("syswrite -c wrote -o \"$_nova_req_fd\" -- \"$frame\""));
        assert!(script.contains("(( wrote != frame_len ))"));
    }
}
