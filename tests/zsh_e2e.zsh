#!/usr/bin/env zsh

emulate -LR zsh
setopt err_exit no_unset pipe_fail
zmodload zsh/datetime zsh/zpty zsh/zselect

typeset -gr script_dir=${0:A:h}
typeset -gr repo_root=${script_dir:h}
typeset -gr nova_bin=${NOVA_E2E_BIN:-$repo_root/target/debug/nova}
typeset -gr zsh_bin=${commands[zsh]}
typeset -gr main_marker='NOVA_E2E_MAIN>'
typeset -gr vi_marker='NOVA_E2E_VICMD>'
typeset -gr branch_marker='nova-e2e-branch'
typeset -gr e2e_root=$(mktemp -d "${TMPDIR:-/tmp}/nova-zsh-e2e.XXXXXX")
typeset -gr home_dir=$e2e_root/home
typeset -gr temp_dir=$e2e_root/tmp
typeset -gr state_dir=$e2e_root/state
typeset -gr git_repo=$e2e_root/repo
typeset -gr config_path=$e2e_root/config.toml

typeset -g transcript=
typeset -gi output_checkpoint=0
typeset -gi pty_fd=-1
typeset -gi session_active=0
typeset -g shell_pid=
typeset -g worker_pid=
typeset -g runtime_dir=

fail() {
  print -ru2 -- "zsh E2E: $*"
  if [[ -n $transcript ]]; then
    print -ru2 -- '--- terminal transcript ---'
    print -ru2 -- "${transcript//$'\r'/}"
    print -ru2 -- '--- end transcript ---'
  fi
  exit 1
}

cleanup() {
  local -i exit_status=$?
  setopt local_options no_err_exit

  if (( session_active )); then
    zpty -d nova_e2e >/dev/null 2>&1 || true
  fi
  if [[ $worker_pid == <-> ]] && kill -0 "$worker_pid" 2>/dev/null; then
    kill "$worker_pid" 2>/dev/null || true
  fi
  command rm -rf -- "$e2e_root"
  return $exit_status
}

drain_output() {
  local chunk
  while zpty -rt nova_e2e chunk; do
    transcript+=$chunk
  done
}

checkpoint_output() {
  drain_output
  output_checkpoint=${#transcript}
}

output_since_checkpoint() {
  local -i start=$(( output_checkpoint + 1 ))
  REPLY=${transcript[$start,-1]}
}

wait_for_output() {
  local expected=$1
  local description=$2
  local -F timeout_seconds=${3:-10}
  local -F deadline=$(( EPOCHREALTIME + timeout_seconds ))
  local recent

  while (( EPOCHREALTIME < deadline )); do
    drain_output
    output_since_checkpoint
    recent=$REPLY
    [[ $recent == *"$expected"* ]] && return 0
    zselect -t 5 -r $pty_fd >/dev/null 2>&1 || true
  done

  fail "timed out waiting for $description (${(qqq)expected})"
}

wait_for_file() {
  local path=$1
  local -F deadline=$(( EPOCHREALTIME + 10 ))

  while (( EPOCHREALTIME < deadline )); do
    [[ -s $path ]] && return 0
    zselect -t 5 >/dev/null 2>&1 || true
  done

  fail "timed out waiting for state file ${(qqq)path}"
}

wait_for_shell_exit() {
  local -F deadline=$(( EPOCHREALTIME + 10 ))

  while (( EPOCHREALTIME < deadline )); do
    zpty -t nova_e2e || return 0
    zselect -t 5 >/dev/null 2>&1 || true
  done

  fail "shell $shell_pid survived SIGKILL"
}

wait_for_worker_cleanup() {
  local -F deadline=$(( EPOCHREALTIME + 10 ))

  while (( EPOCHREALTIME < deadline )); do
    if ! kill -0 "$worker_pid" 2>/dev/null && [[ ! -e $runtime_dir ]]; then
      return 0
    fi
    zselect -t 5 >/dev/null 2>&1 || true
  done

  fail "worker $worker_pid or runtime directory ${(qqq)runtime_dir} survived shell termination"
}

trap cleanup EXIT
trap 'exit 130' HUP INT TERM

[[ -x $nova_bin ]] || fail "nova binary not found: $nova_bin (run cargo build first)"
command mkdir -p -- "$home_dir" "$temp_dir" "$state_dir" "$git_repo"

{
  print -r -- '[async]'
  print -r -- 'initial_wait_ms = 0'
  print -r -- 'min_loading_ms = 0'
  print -r -- ''
  print -r -- '[layout]'
  print -r -- 'lines = 1'
  print -r -- ''
  print -r -- '[layout.line1]'
  print -r -- 'left = ["dir", "git_branch", "prompt_char"]'
  print -r -- 'right = []'
  print -r -- ''
  print -r -- '[segments.git_branch]'
  print -r -- 'icon = ""'
  # Keep the result beyond precmd's read window so the ZLE redraw is observable.
  print -r -- 'min_loading_ms = 750'
  print -r -- ''
  print -r -- '[segments.prompt_char]'
  print -r -- 'character = "NOVA_E2E_MAIN>"'
  print -r -- ''
  print -r -- '[segments.prompt_char.characters]'
  print -r -- 'main = "NOVA_E2E_MAIN>"'
  print -r -- 'vi_command = "NOVA_E2E_VICMD>"'
} >| "$config_path"

command git init --quiet "$git_repo"
command git -C "$git_repo" checkout --quiet -b "$branch_marker"
print -r -- 'fixture' >| "$git_repo/fixture.txt"
command git -C "$git_repo" add fixture.txt
command git -C "$git_repo" -c user.name=Nova -c user.email=nova@example.invalid \
  commit --quiet -m initial

export HOME=$home_dir
export TMPDIR=$temp_dir
export NOVA_CONFIG=$config_path
export NOVA_E2E_BIN=$nova_bin
export NOVA_E2E_STATE=$state_dir
export PATH="${nova_bin:h}:$PATH"
export TERM=xterm-256color
unset XDG_RUNTIME_DIR SSH_CLIENT SSH_CONNECTION SSH_TTY

zpty -b nova_e2e "$zsh_bin" -f
pty_fd=$REPLY
session_active=1

checkpoint_output
zpty -w nova_e2e 'eval "$("$NOVA_E2E_BIN" init zsh)"'
wait_for_output "$main_marker" 'the first Nova prompt'
print -r -- 'ok 1 - first prompt renders without fallback'

checkpoint_output
zpty -w nova_e2e 'print -r -- "$$" >| "$NOVA_E2E_STATE/shell.pid"; print -r -- "$_nova_worker_pid" >| "$NOVA_E2E_STATE/worker.pid"; print -r -- "$_nova_runtime_dir" >| "$NOVA_E2E_STATE/runtime-dir"'
wait_for_output "$main_marker" 'the prompt after capturing process state'
wait_for_file "$state_dir/shell.pid"
wait_for_file "$state_dir/worker.pid"
wait_for_file "$state_dir/runtime-dir"

shell_pid=$(<"$state_dir/shell.pid")
worker_pid=$(<"$state_dir/worker.pid")
runtime_dir=$(<"$state_dir/runtime-dir")
[[ $shell_pid == <-> ]] || fail "invalid shell pid: ${(qqq)shell_pid}"
[[ $worker_pid == <-> ]] || fail "invalid worker pid: ${(qqq)worker_pid}"
kill -0 "$worker_pid" 2>/dev/null || fail "worker $worker_pid is not running"
[[ $runtime_dir == "$temp_dir/nova-$shell_pid" ]] || \
  fail "unexpected runtime directory: ${(qqq)runtime_dir}"
[[ -d $runtime_dir ]] || fail "runtime directory does not exist: ${(qqq)runtime_dir}"

checkpoint_output
zpty -w nova_e2e "cd -- ${(q)git_repo}"
wait_for_output "$branch_marker" 'the asynchronous git branch redraw'
output_since_checkpoint
# The branch precedes the prompt character, so this order proves there were two renders.
[[ $REPLY == *"$main_marker"*"$branch_marker"* ]] || \
  fail 'git branch appeared before the initial prompt instead of through a later redraw'
print -r -- 'ok 2 - git branch redraws asynchronously after cd'

checkpoint_output
zpty -w nova_e2e 'bindkey -v'
wait_for_output "$main_marker" 'the vi insert-mode prompt'
checkpoint_output
zpty -w -n nova_e2e $'\e'
wait_for_output "$vi_marker" 'the vi command-mode prompt redraw'
print -r -- 'ok 3 - keymap switch redraws the prompt character'

kill -KILL "$shell_pid"
wait_for_shell_exit
session_active=0
wait_for_worker_cleanup
print -r -- 'ok 4 - killed shell leaves no worker or runtime directory'
