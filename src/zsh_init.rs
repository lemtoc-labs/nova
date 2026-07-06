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
}
