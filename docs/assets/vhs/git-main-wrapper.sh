#!/bin/sh
if [ "${2:-}" = "symbolic-ref" ]; then
  printf 'main\n'
  exit 0
fi

exec "${NOVA_VHS_REAL_GIT:?}" "$@"
