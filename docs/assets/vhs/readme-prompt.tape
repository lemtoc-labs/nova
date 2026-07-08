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
Type "export NOVA_VHS_REAL_GIT=$(command -v git)"
Enter
Type "export NOVA_VHS_BIN=$(mktemp -d)"
Enter
Type "cp $NOVA_VHS_PROJECT/docs/assets/vhs/git-main-wrapper.sh $NOVA_VHS_BIN/git && chmod +x $NOVA_VHS_BIN/git"
Enter
Type "export PATH=$NOVA_VHS_BIN:$NOVA_VHS_PROJECT/target/debug:$PATH"
Enter
Type "printf demo-stash >> README.md && git stash push -m nova-vhs-stash -- README.md && printf staged > .nova-vhs-status && git add .nova-vhs-status && printf modified >> .nova-vhs-status"
Enter
Type "cd ~"
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
Sleep 2s
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
