#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# Use the packaging harness from the exact Cargo-resolved DeskQuest revision.
XR_DIR="$(cd "$ROOT" && cargo metadata --format-version 1 --locked | python3 -c 'import json,sys,os; d=json.load(sys.stdin); print(os.path.dirname(next(p["manifest_path"] for p in d["packages"] if p["name"] == "xr-core")))')"
export APP_DIR="$ROOT/crates/quest-app" APP_LIB_NAME=quest_app APP_PACKAGE=org.questjellyfin.player APP_LABEL="Jellyfin 3D" APP_APK_NAME=jellyfin-3d
export APP_ACTIVITY=org.questjellyfin.player.MainActivity APP_MANIFEST="$HERE/AndroidManifest.xml" APP_RES_DIR="$HERE/res" APP_JAVA_DIR="$HERE/src"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
exec bash "$XR_DIR/android/build.sh" "$@"
