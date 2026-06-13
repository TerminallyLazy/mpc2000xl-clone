#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"

cd "$repo_root"

cargo fmt --all --check
cargo test --workspace
cargo check -p mpc_desktop
python3 tools/check_assets.py
