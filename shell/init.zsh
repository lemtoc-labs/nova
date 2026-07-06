# shellcheck shell=zsh

if [[ -n ${_NOVA_BOOTSTRAP_ZSH_LOADED:-} ]]; then
  return
fi
typeset -g _NOVA_BOOTSTRAP_ZSH_LOADED=1

zmodload zsh/datetime 2>/dev/null || true
autoload -Uz add-zsh-hook

typeset -g _nova_bin=@NOVA_BIN@
typeset -g _nova_cmd_start=

_nova_preexec() {
  emulate -L zsh
  _nova_cmd_start=${EPOCHREALTIME:-}
}

_nova_precmd() {
  local exit_status=$?
  emulate -L zsh

  local -a args
  local -i columns=${COLUMNS:-80}
  if (( columns <= 0 )); then
    columns=80
  fi

  args=(
    prompt
    --format shell
    --cwd "$PWD"
    --cols "$columns"
    --exit "$exit_status"
    --keymap "${KEYMAP:-main}"
  )

  if [[ -n ${_nova_cmd_start:-} && -n ${EPOCHREALTIME:-} ]]; then
    local -i duration_ms
    duration_ms=$(( (EPOCHREALTIME - _nova_cmd_start) * 1000 ))
    args+=(--duration-ms "$duration_ms")
  fi

  local rendered
  rendered="$("$_nova_bin" "${args[@]}" 2>/dev/null)"
  if [[ $? -eq 0 && -n "$rendered" ]]; then
    eval "$rendered"
  else
    PROMPT='%~ %# '
    RPROMPT=''
  fi

  _nova_cmd_start=
}

add-zsh-hook preexec _nova_preexec
add-zsh-hook precmd _nova_precmd
