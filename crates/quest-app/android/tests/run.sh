#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/AudioTrackSelector.java" "$HERE/../src/org/questjellyfin/player/SurfacePreparation.java" "$HERE/AudioTrackSelectorTest.java" "$HERE/SurfacePreparationTest.java" "$HERE/../src/org/questjellyfin/player/QuestPreferences.java" "$HERE/QuestPreferencesTest.java"
java -cp "$OUT" AudioTrackSelectorTest
java -cp "$OUT" org.questjellyfin.player.SurfacePreparationTest

java -cp "$OUT" org.questjellyfin.player.QuestPreferencesTest
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/PlaybackGate.java" "$HERE/PlaybackGateTest.java"
java -cp "$OUT" org.questjellyfin.player.PlaybackGateTest
