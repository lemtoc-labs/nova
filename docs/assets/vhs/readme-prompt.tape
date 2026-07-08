# Nova README async prompt demo.
Source docs/assets/vhs/config.tape
Output docs/assets/vhs/readme-prompt.gif
Require nix

Hide
Type "unset NO_COLOR IN_NIX_SHELL name NIX_SHELL_LEVEL"
Enter
Type "export TERM=xterm-256color COLORTERM=truecolor CLICOLOR=1 CLICOLOR_FORCE=1 FORCE_COLOR=1"
Enter
Type "export NOVA_VHS_PROJECT=$PWD"
Enter
Type "export NOVA_CONFIG=$NOVA_VHS_PROJECT/docs/assets/vhs/async-demo.toml"
Enter
Type "nix develop -c cargo build -q"
Enter
Sleep 1s
Type "export PATH=$NOVA_VHS_PROJECT/target/debug:$PATH"
Enter
Type "export NOVA_VHS_REAL_GIT=$(command -v git)"
Enter
Type "export NOVA_VHS_REAL_RUSTC=$(command -v rustc)"
Enter
Type "export PATH=$NOVA_VHS_PROJECT/docs/assets/vhs/bin:$PATH"
Enter
Type "cd ~"
Enter
Type "chpwd() { if [[ $PWD == $NOVA_VHS_PROJECT ]]; then export IN_NIX_SHELL=impure name=nix-shell-env NIX_SHELL_LEVEL=1; else unset IN_NIX_SHELL name NIX_SHELL_LEVEL; fi }"
Enter
Type "nova init zsh > /tmp/nova-init.zsh"
Enter
Type "source /tmp/nova-init.zsh"
Enter
Sleep 1s
Type "clear"
Enter
Sleep 500ms
Show
Sleep 1s

Type "cd dev/oss/nova"
Sleep 0.25s
Enter
Sleep 3s
Type "false"
Sleep 0.25s
Enter
Sleep 1.5s
Type "true"
Sleep 0.25s
Enter
Sleep 1.5s
Type "sleep 2s"
Sleep 0.25s
Enter
Sleep 5s
