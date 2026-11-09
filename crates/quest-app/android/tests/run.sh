#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/AudioTrackSelector.java" "$HERE/../src/org/questjellyfin/player/SubtitleTrackSelector.java" "$HERE/../src/org/questjellyfin/player/SrtSubtitles.java" "$HERE/../src/org/questjellyfin/player/SurfacePreparation.java" "$HERE/AudioTrackSelectorTest.java" "$HERE/SubtitleTrackSelectorTest.java" "$HERE/SrtSubtitlesTest.java" "$HERE/SurfacePreparationTest.java" "$HERE/../src/org/questjellyfin/player/QuestPreferences.java" "$HERE/QuestPreferencesTest.java"
java -cp "$OUT" AudioTrackSelectorTest
java -cp "$OUT" SubtitleTrackSelectorTest
java -cp "$OUT" SrtSubtitlesTest
javac -cp "$OUT" -d "$OUT" "$HERE/SubtitlePlaybackIntegrationTest.java"
java -cp "$OUT" SubtitlePlaybackIntegrationTest
java -cp "$OUT" org.questjellyfin.player.SurfacePreparationTest

java -cp "$OUT" org.questjellyfin.player.QuestPreferencesTest
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/PlaybackGate.java" "$HERE/PlaybackGateTest.java"
java -cp "$OUT" org.questjellyfin.player.PlaybackGateTest
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/PlaybackChrome.java" "$HERE/PlaybackChromeTest.java"
java -cp "$OUT" org.questjellyfin.player.PlaybackChromeTest
javac -d "$OUT" "$HERE/../src/org/questjellyfin/player/VideoPresentation.java" "$HERE/VideoPresentationTest.java"
java -cp "$OUT" org.questjellyfin.player.VideoPresentationTest
javac --add-modules jdk.httpserver -d "$OUT" "$HERE/../src/org/questjellyfin/player/HttpRangeReader.java" "$HERE/HttpRangeReaderTest.java"
java --add-modules jdk.httpserver -cp "$OUT" org.questjellyfin.player.HttpRangeReaderTest
