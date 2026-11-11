#!/bin/bash

launch_app() { /usr/bin/open "$1" --args "${@:2}"; }
show_update_failure() {
  /usr/bin/osascript -e 'display alert "Krill Desktop 更新失败" message "已恢复原版本，请重新尝试更新。"' || true
}

install_update() (
  set -euo pipefail
  parent_pid="$1"
  destination="$2"
  staging="$3"
  shift 3
  backup="$staging/previous.app"
  payload="$staging/Krill Desktop.app"
  log_file="${TMPDIR:-/tmp/}krill-update.log"
  exec >>"$log_file" 2>&1

  # Wait for the owning application to release its executable; never terminate it forcibly.
  for ((attempt = 0; attempt < 120; attempt++)); do
    if ! kill -0 "$parent_pid" 2>/dev/null; then break; fi
    sleep 0.5
  done
  if kill -0 "$parent_pid" 2>/dev/null; then
    echo "Update cancelled: application did not exit."
    rm -rf "$staging"
    exit 1
  fi
  rollback() {
    if [[ -d "$backup" ]]; then
      rm -rf "$destination"
      mv "$backup" "$destination"
    fi
    launch_app "$destination" "$@" || true
    show_update_failure
  }
  if ! mv "$destination" "$backup"; then rollback "$@"; exit 1; fi
  if ! mv "$payload" "$destination"; then rollback "$@"; exit 1; fi
  if ! launch_app "$destination" "$@"; then rollback "$@"; exit 1; fi
  rm -rf "$staging"
)

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then install_update "$@"; fi
