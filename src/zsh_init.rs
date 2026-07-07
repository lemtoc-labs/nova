//! zsh adapter script embedding.

use std::path::Path;

const INIT_ZSH: &str = include_str!("../shell/init.zsh");

pub fn render_init_script(binary_path: &Path) -> String {
    INIT_ZSH.replace(
        "@NOVA_BIN@",
        &zsh_single_quote(&binary_path.to_string_lossy()),
    )
}

fn zsh_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_binary_path_as_a_single_quoted_zsh_string() {
        let script = render_init_script(Path::new("/tmp/nova bin"));

        assert!(script.contains("typeset -g _nova_bin='/tmp/nova bin'"));
    }

    #[test]
    fn escapes_single_quotes_in_binary_paths() {
        let script = render_init_script(Path::new("/tmp/no'va"));

        assert!(script.contains("typeset -g _nova_bin='/tmp/no'\\''va'"));
    }

    #[test]
    fn registers_zle_line_init_for_pending_updates() {
        let script = render_init_script(Path::new("/tmp/nova"));

        assert!(script.contains("autoload -Uz add-zsh-hook add-zle-hook-widget"));
        assert!(script.contains("add-zle-hook-widget line-init _nova_zle_line_init"));
    }

    #[test]
    fn sends_virtual_env_in_render_requests() {
        let script = render_init_script(Path::new("/tmp/nova"));

        assert!(script.contains(
            "${KEYMAP:-main}${_nova_nul}${USER:-}${_nova_nul}${prompt_host}${_nova_nul}${prompt_time}${_nova_nul}${VIRTUAL_ENV:-}"
        ));
    }

    #[test]
    fn sends_nix_shell_env_in_render_requests() {
        let script = render_init_script(Path::new("/tmp/nova"));

        assert!(
            script
                .contains("${IN_NIX_SHELL:-}${_nova_nul}${name:-}${_nova_nul}${NIX_SHELL_LEVEL:-}")
        );
    }

    #[test]
    fn sends_aws_env_in_render_requests_without_secret_values() {
        let script = render_init_script(Path::new("/tmp/nova"));

        assert!(
            script.contains(
                "${HOME:-}${_nova_nul}${AWSU_PROFILE:-}${_nova_nul}${AWS_VAULT:-}${_nova_nul}${AWSUME_PROFILE:-}${_nova_nul}${AWS_PROFILE:-}${_nova_nul}${AWS_SSO_PROFILE:-}${_nova_nul}${AWS_REGION:-}${_nova_nul}${AWS_DEFAULT_REGION:-}${_nova_nul}${AWS_CONFIG_FILE:-}${_nova_nul}${AWS_SHARED_CREDENTIALS_FILE:-}${_nova_nul}${AWS_CREDENTIALS_FILE:-}${_nova_nul}${AWS_ACCESS_KEY_ID:+1}${_nova_nul}${AWS_SECRET_ACCESS_KEY:+1}${_nova_nul}${AWS_SESSION_TOKEN:+1}"
            )
        );
    }
}
