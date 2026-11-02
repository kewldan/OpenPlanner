#!/usr/bin/env bash
# Сборка WASM-движка (op-engine) и JS-обвязки wasm-bindgen в packages/engine-client/wasm.
set -euo pipefail

cd "$(dirname "$0")/.."

profile="dev"
target_dir="debug"
if [[ "${1:-}" == "--release" ]]; then
  profile="wasm-release"
  target_dir="wasm-release"
fi

expected=$(grep -E '^wasm-bindgen = ' Cargo.toml | sed -E 's/.*"=?([0-9.]+)".*/\1/')
if ! command -v wasm-bindgen >/dev/null || [[ "$(wasm-bindgen --version | awk '{print $2}')" != "$expected" ]]; then
  echo "Нужен wasm-bindgen-cli $expected:" >&2
  echo "  cargo install wasm-bindgen-cli --version $expected --locked" >&2
  exit 1
fi

cargo build -p op-engine --target wasm32-unknown-unknown --profile "$profile"
wasm-bindgen \
  --target web \
  --out-dir packages/engine-client/wasm \
  --out-name op_engine \
  "target/wasm32-unknown-unknown/$target_dir/op_engine.wasm"
