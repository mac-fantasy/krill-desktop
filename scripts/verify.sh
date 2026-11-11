#!/bin/bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
cargo fmt --all --check
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
cargo build --locked --package krill-desktop --features native-qa
./target/debug/KrillMac --smoke-test --data-dir "${TMPDIR:-/tmp}/krill-native-demo" --output artifacts/rust-native
