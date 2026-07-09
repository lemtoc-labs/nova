# shellcheck shell=zsh

if [[ -n ${_NOVA_ZSH_LOADED:-} ]]; then
  return
fi
typeset -g _NOVA_ZSH_LOADED=1

zmodload zsh/datetime zsh/system zsh/zselect 2>/dev/null || true
autoload -Uz add-zsh-hook add-zle-hook-widget

typeset -g _nova_bin=@NOVA_BIN@
typeset -g _nova_cmd_start=
typeset -g _nova_runtime_dir="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/nova-$$"
typeset -g _nova_req_fifo="$_nova_runtime_dir/req"
typeset -g _nova_resp_fifo="$_nova_runtime_dir/resp"
typeset -g _nova_session_token=
typeset -g _nova_req_fd=
typeset -g _nova_resp_fd=
typeset -g _nova_worker_pid=
typeset -g _nova_resp_buffer=
typeset -g _nova_gen=0
typeset -g _nova_last_applied_gen=0
typeset -g _nova_reply_applied=0
typeset -g _nova_reply_status=
typeset -g _nova_wait_cs=0
typeset -g _nova_handshake_ok=0
typeset -g _nova_failures=0
typeset -g _nova_warned_dead=0
typeset -g _nova_nul=$'\0'
typeset -g _nova_rs=$'\x1e'
typeset -g _nova_protocol_version=@NOVA_PROTOCOL_VERSION@

_nova_generate_session_token() {
  emulate -L zsh
  local token=
  if [[ -r /dev/urandom ]]; then
    token=$(command od -An -N16 -tx1 /dev/urandom 2>/dev/null | command tr -d '[:space:]') || token=
  fi
  if [[ -n "$token" ]]; then
    print -r -- "$token-$$"
  else
    print -r -- "${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}-$$"
  fi
}

_nova_session_token="$(_nova_generate_session_token)"

_nova_setup_runtime() {
  emulate -L zsh
  command rm -rf -- "$_nova_runtime_dir" 2>/dev/null || true
  command mkdir -m 700 -- "$_nova_runtime_dir" || return 1
  [[ -d "$_nova_runtime_dir" && -O "$_nova_runtime_dir" ]] || return 1
  command chmod 700 "$_nova_runtime_dir" 2>/dev/null || {
    command rm -rf -- "$_nova_runtime_dir" 2>/dev/null || true
    return 1
  }
  local old_umask
  old_umask=$(umask)
  umask 077
  command mkfifo -- "$_nova_req_fifo" "$_nova_resp_fifo"
  local -i mkfifo_status=$?
  umask "$old_umask"
  if (( mkfifo_status != 0 )); then
    command rm -rf -- "$_nova_runtime_dir" 2>/dev/null || true
  fi
  return $mkfifo_status
}

_nova_spawn_worker() {
  emulate -L zsh
  _nova_worker_alive && return 0
  unsetopt bg_nice 2>/dev/null || true
  [[ -p "$_nova_req_fifo" && -p "$_nova_resp_fifo" ]] || _nova_setup_runtime || return 1
  NOVA_SESSION_TOKEN="$_nova_session_token" NOVA_PARENT_PID=$$ \
    "$_nova_bin" worker --dir "$_nova_runtime_dir" \
    </dev/null >/dev/null 2>/dev/null &!
  _nova_worker_pid=$!
}

_nova_worker_alive() {
  emulate -L zsh
  [[ -n ${_nova_worker_pid:-} ]] || return 1
  kill -0 "$_nova_worker_pid" 2>/dev/null
}

_nova_close_fds() {
  emulate -L zsh
  if [[ -n ${_nova_resp_fd:-} ]]; then
    if [[ -o interactive ]]; then
      zle -F "$_nova_resp_fd" 2>/dev/null || true
    fi
    exec {_nova_resp_fd}<&- 2>/dev/null || true
  fi
  if [[ -n ${_nova_req_fd:-} ]]; then
    exec {_nova_req_fd}>&- 2>/dev/null || true
  fi
  _nova_req_fd=
  _nova_resp_fd=
  _nova_resp_buffer=
  _nova_handshake_ok=0
}

_nova_mark_dead() {
  emulate -L zsh
  _nova_close_fds
  (( _nova_failures++ ))
}

_nova_open_transport() {
  emulate -L zsh
  [[ -n ${_nova_req_fd:-} && -n ${_nova_resp_fd:-} ]] && return 0

  sysopen -o nonblock -o cloexec -w -u _nova_req_fd -- "$_nova_req_fifo" 2>/dev/null
  if [[ $? -ne 0 ]]; then
    zselect -t 1 2>/dev/null || true
    sysopen -o nonblock -o cloexec -w -u _nova_req_fd -- "$_nova_req_fifo" 2>/dev/null || return 1
  fi

  sysopen -o nonblock -o cloexec -r -u _nova_resp_fd -- "$_nova_resp_fifo" 2>/dev/null || {
    _nova_close_fds
    return 1
  }

  _nova_register_update_handler

  zselect -t 5 -r "$_nova_resp_fd" >/dev/null 2>&1 || {
    _nova_close_fds
    return 1
  }

  _nova_drain || return 1
  (( _nova_handshake_ok )) || {
    _nova_close_fds
    return 1
  }
}

_nova_register_update_handler() {
  emulate -L zsh
  [[ -n ${_nova_resp_fd:-} ]] || return 0
  [[ -o interactive ]] || return 0
  zle -F "$_nova_resp_fd" _nova_on_update 2>/dev/null || true
}

_nova_ensure_worker() {
  emulate -L zsh
  (( _nova_failures >= 3 )) && return 1

  if [[ -n ${_nova_req_fd:-} && -n ${_nova_resp_fd:-} ]]; then
    return 0
  fi

  if ! _nova_worker_alive; then
    _nova_spawn_worker || {
      _nova_mark_dead
      return 1
    }
  fi

  _nova_open_transport || {
    _nova_mark_dead
    return 1
  }
}

_nova_send_request() {
  emulate -L zsh
  local exit_status=$1
  local duration_ms=$2
  local -i columns=${COLUMNS:-80}
  if (( columns <= 0 )); then
    columns=80
  fi
  local prompt_time=
  strftime -s prompt_time '%H:%M:%S' ${EPOCHSECONDS:-0} 2>/dev/null || prompt_time=
  local prompt_host=${HOST:-${HOSTNAME:-}}

  local frame="R${_nova_nul}${_nova_gen}${_nova_nul}${PWD}${_nova_nul}${exit_status}${_nova_nul}${duration_ms}${_nova_nul}${columns}${_nova_nul}${KEYMAP:-main}${_nova_nul}${USER:-}${_nova_nul}${prompt_host}${_nova_nul}${prompt_time}${_nova_nul}${VIRTUAL_ENV:-}${_nova_nul}${IN_NIX_SHELL:-}${_nova_nul}${name:-}${_nova_nul}${NIX_SHELL_LEVEL:-}${_nova_nul}${HOME:-}${_nova_nul}${AWSU_PROFILE:-}${_nova_nul}${AWS_VAULT:-}${_nova_nul}${AWSUME_PROFILE:-}${_nova_nul}${AWS_PROFILE:-}${_nova_nul}${AWS_SSO_PROFILE:-}${_nova_nul}${AWS_REGION:-}${_nova_nul}${AWS_DEFAULT_REGION:-}${_nova_nul}${AWS_CONFIG_FILE:-}${_nova_nul}${AWS_SHARED_CREDENTIALS_FILE:-}${_nova_nul}${AWS_CREDENTIALS_FILE:-}${_nova_nul}${AWS_ACCESS_KEY_ID:+1}${_nova_nul}${AWS_SECRET_ACCESS_KEY:+1}${_nova_nul}${AWS_SESSION_TOKEN:+1}${_nova_nul}${PATH:-}${_nova_rs}"
  local -i wrote=0 frame_len=0
  () { setopt localoptions no_multibyte; frame_len=${#1} } "$frame"
  if ! syswrite -c wrote -o "$_nova_req_fd" -- "$frame" 2>/dev/null \
      || (( wrote != frame_len )); then
    return 1
  fi
}

_nova_drain() {
  emulate -L zsh
  local chunk

  while zselect -t 0 -r "$_nova_resp_fd" >/dev/null 2>&1; do
    chunk=
    sysread -i "$_nova_resp_fd" -s 4096 chunk 2>/dev/null || return 1
    [[ -n "$chunk" ]] || return 1
    _nova_resp_buffer+="$chunk"
  done

  _nova_process_buffer
}

_nova_process_buffer() {
  emulate -L zsh
  local record
  while [[ "$_nova_resp_buffer" == *"$_nova_rs"* ]]; do
    record="${_nova_resp_buffer%%$_nova_rs*}"
    _nova_resp_buffer="${_nova_resp_buffer#*$_nova_rs}"
    _nova_apply_record "$record"
  done
}

_nova_apply_record() {
  emulate -L zsh
  local record=$1
  local -a fields
  fields=("${(0)record}")

  case "${fields[1]}" in
    H)
      if [[ "${fields[2]}" == "$_nova_protocol_version" && "${fields[3]}" == "$_nova_session_token" ]]; then
        _nova_handshake_ok=1
        _nova_wait_cs=$(( (${fields[4]:-0} + 9) / 10 ))
      fi
      ;;
    P|U)
      local -i gen=${fields[2]:-0}
      (( gen < _nova_last_applied_gen )) && return
      _nova_last_applied_gen=$gen
      _nova_reply_status="${fields[3]}"
      PROMPT="${fields[4]}"
      RPROMPT="${fields[5]}"
      _nova_reply_applied=1
      ;;
  esac
}

_nova_fallback() {
  emulate -L zsh
  PROMPT='%~ %# '
  RPROMPT=''
  if (( _nova_failures >= 3 && ! _nova_warned_dead )); then
    print -ru2 -- 'nova: worker unavailable; using fallback prompt for this session'
    _nova_warned_dead=1
  fi
}

_nova_on_update() {
  emulate -L zsh
  _nova_drain_and_redraw
}

_nova_drain_and_redraw() {
  emulate -L zsh
  _nova_reply_applied=0
  _nova_drain || {
    _nova_mark_dead
    return 1
  }
  if (( _nova_reply_applied )); then
    zle reset-prompt 2>/dev/null || true
  fi
}

_nova_zle_line_init() {
  emulate -L zsh
  _nova_register_update_handler
  [[ -n ${_nova_resp_fd:-} ]] || return 0
  _nova_drain_and_redraw
}

_nova_preexec() {
  emulate -L zsh
  _nova_cmd_start=${EPOCHREALTIME:-}
}

_nova_precmd() {
  local exit_status=$?
  emulate -L zsh

  local duration_ms=
  if [[ -n ${_nova_cmd_start:-} && -n ${EPOCHREALTIME:-} ]]; then
    local -i elapsed_ms
    elapsed_ms=$(( (EPOCHREALTIME - _nova_cmd_start) * 1000 ))
    duration_ms=$elapsed_ms
  fi

  _nova_ensure_worker || {
    _nova_fallback
    _nova_cmd_start=
    return
  }

  (( _nova_gen++ ))
  _nova_reply_applied=0
  _nova_reply_status=
  _nova_send_request "$exit_status" "$duration_ms" || {
    _nova_mark_dead
    _nova_fallback
    _nova_cmd_start=
    return
  }

  if zselect -t 5 -r "$_nova_resp_fd" >/dev/null 2>&1; then
    _nova_drain || {
      _nova_mark_dead
      _nova_fallback
      _nova_cmd_start=
      return
    }
  fi

  if [[ "$_nova_reply_status" == partial ]] && (( _nova_wait_cs > 0 )); then
    local -F deadline=$(( EPOCHREALTIME + _nova_wait_cs / 100.0 ))
    local -i remaining_cs
    while [[ "$_nova_reply_status" != final ]]; do
      remaining_cs=$(( (deadline - EPOCHREALTIME) * 100 ))
      (( remaining_cs > 0 )) || break
      zselect -t $remaining_cs -r "$_nova_resp_fd" >/dev/null 2>&1 || break
      _nova_drain || {
        _nova_mark_dead
        _nova_fallback
        _nova_cmd_start=
        return
      }
    done
  fi

  if (( ! _nova_reply_applied )); then
    PROMPT='%~ %# '
    RPROMPT=''
  fi

  _nova_cmd_start=
}

_nova_cleanup() {
  emulate -L zsh
  _nova_close_fds
  command rm -rf -- "$_nova_runtime_dir" 2>/dev/null || true
}

if [[ -o interactive ]]; then
  _nova_spawn_worker || true
fi

add-zsh-hook preexec _nova_preexec
add-zsh-hook precmd _nova_precmd
add-zsh-hook zshexit _nova_cleanup
add-zle-hook-widget line-init _nova_zle_line_init 2>/dev/null || true
