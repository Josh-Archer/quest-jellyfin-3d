#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
SDK="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
BUILD_TOOLS="$(find "$SDK/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
JAR="$SDK/platforms/android-34/android.jar"
OUT="$(mktemp -d)"
ARTIFACTS="${QUEST_SMOKE_ARTIFACTS:-$ROOT/crates/quest-app/target/device-smoke}"
PACKAGE=org.questjellyfin.player.smoke
RUNNER=org.questjellyfin.tests
SERVER_PID=""
PORT=""
cleanup() {
    if [[ -n "$SERVER_PID" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
    if [[ -n "$PORT" ]]; then adb reverse --remove "tcp:$PORT" >/dev/null 2>&1 || true; fi
    adb uninstall "$RUNNER" >/dev/null 2>&1 || true
    adb uninstall "$PACKAGE" >/dev/null 2>&1 || true
    rm -rf "$OUT"
}
trap cleanup EXIT
mkdir -p "$ARTIFACTS"
ffmpeg -v error -f lavfi -i testsrc2=size=640x360:rate=30 -t 30 -c:v libx264 -pix_fmt yuv420p -movflags +faststart "$OUT/video.mp4"
python3 "$HERE/device/mock_server.py" "$OUT/video.mp4" "$OUT/port" &
SERVER_PID=$!
for attempt in {1..50}; do if [[ -s "$OUT/port" ]]; then break; fi; sleep 0.1; done
PORT="$(cat "$OUT/port")"
adb reverse "tcp:$PORT" "tcp:$PORT"
python3 - "$ROOT/crates/quest-app/android/AndroidManifest.xml" "$OUT/AndroidManifest.xml" <<'PY'
import sys
from pathlib import Path
source = Path(sys.argv[1]).read_text().replace('package="org.questjellyfin.player"', 'package="org.questjellyfin.player.smoke"').replace('android:label="Jellyfin 3D"', 'android:label="Jellyfin UI Test"').replace('taskAffinity="org.questjellyfin.player.cinema"', 'taskAffinity="org.questjellyfin.player.smoke.cinema"')
Path(sys.argv[2]).write_text(source)
PY
XR_DIR="$(cd "$ROOT" && cargo metadata --format-version 1 --locked | python3 -c 'import json,sys,os; print(os.path.dirname(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"]=="xr-core")))')"
export APP_DIR="$ROOT/crates/quest-app" APP_LIB_NAME=quest_app APP_PACKAGE="$PACKAGE" APP_LABEL="Jellyfin UI Test" APP_APK_NAME=jellyfin-ui-smoke
export APP_ACTIVITY=org.questjellyfin.player.MainActivity APP_MANIFEST="$OUT/AndroidManifest.xml" APP_RES_DIR="$ROOT/crates/quest-app/android/res" APP_JAVA_DIR="$ROOT/crates/quest-app/android/src" APP_APK_OUT_DIR="$OUT/app"
export CARGO_TARGET_DIR="$ROOT/target"
export APP_KEYSTORE="$HOME/.local/share/org.questjellyfin.player/debug.keystore"
bash "$XR_DIR/android/build.sh" > "$ARTIFACTS/build.log" 2>&1
adb install -r "$OUT/app/jellyfin-ui-smoke-debug.apk"
mkdir -p "$OUT/test"
cat > "$OUT/test/AndroidManifest.xml" <<'XML'
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="org.questjellyfin.tests">
 <uses-sdk android:minSdkVersion="32" android:targetSdkVersion="34" />
 <application android:label="Jellyfin UI checks" />
 <instrumentation android:name="org.questjellyfin.tests.DeviceSmoke" android:targetPackage="org.questjellyfin.player.smoke" />
</manifest>
XML
mkdir -p "$OUT/classes" "$OUT/dex"
javac --release 8 -classpath "$JAR" -d "$OUT/classes" "$HERE/device/DeviceSmoke.java"
mapfile -d '' CLASSES < <(find "$OUT/classes" -name '*.class' -print0)
"$BUILD_TOOLS/d8" --lib "$JAR" --min-api 32 --output "$OUT/dex" "${CLASSES[@]}"
"$BUILD_TOOLS/aapt" package -f -M "$OUT/test/AndroidManifest.xml" -I "$JAR" -F "$OUT/test.apk"
(cd "$OUT/dex" && "$BUILD_TOOLS/aapt" add "$OUT/test.apk" classes.dex)
"$BUILD_TOOLS/zipalign" -f 4 "$OUT/test.apk" "$OUT/aligned.apk"
"$BUILD_TOOLS/apksigner" sign --ks "$APP_KEYSTORE" --ks-pass pass:android --out "$OUT/signed.apk" "$OUT/aligned.apk"
adb install -r "$OUT/signed.apk"
adb shell am instrument -w -e server "http://127.0.0.1:$PORT" -e xr_only "${QUEST_SMOKE_XR_ONLY:-false}" org.questjellyfin.tests/org.questjellyfin.tests.DeviceSmoke | tee "$ARTIFACTS/result.txt"
adb pull "/sdcard/Android/data/$PACKAGE/files/." "$ARTIFACTS" >/dev/null
rg -q '^OK:|stream=OK:' "$ARTIFACTS/result.txt"
