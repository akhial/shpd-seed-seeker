#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

wasm_package="$repo_root/web/src/lib/wasm/pkg"
runtime_source="$repo_root/android/app/src/main/assets/third_party/shattered-pixel-dungeon"

mkdir -p "$wasm_package"
wasm-pack build crates/seedfinder-wasm \
    --target web \
    --release \
    --out-dir "$wasm_package" \
    --out-name seedfinder

mkdir -p "$repo_root/web/src/generated"
cp "$runtime_source/catalog-v3.3.8.json" "$repo_root/web/src/generated/catalog.json"

echo "Built browser WASM package in $wasm_package"
echo "Copied generated catalog into web/src/generated/"
