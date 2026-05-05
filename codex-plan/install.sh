#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source_path="$script_dir/codex-plan"
install_dir="${CODEX_PLAN_INSTALL_DIR:-$HOME/.local/bin}"
link_name="${1:-codex-plan}"
target_path="$install_dir/$link_name"

if [[ ! -f "$source_path" ]]; then
  echo "Could not find $source_path" >&2
  exit 1
fi

mkdir -p "$install_dir"
chmod +x "$source_path"
ln -sfn "$source_path" "$target_path"

echo "Installed $target_path -> $source_path"
echo "Make sure $install_dir is on your PATH."
