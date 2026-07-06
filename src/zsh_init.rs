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

        assert!(script.contains("${KEYMAP:-main}${_nova_nul}${VIRTUAL_ENV:-}"));
    }
}
