#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/AudioTrackSelector.java" "$HERE/AudioTrackSelectorTest.java"
java -cp "$OUT" AudioTrackSelectorTest
