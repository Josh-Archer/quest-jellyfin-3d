#!/usr/bin/env python3
"""Prepare a lossless full-SBS Jellyfin version with both MVC eyes and original audio.

Requires edge264-mvc at the pinned revision in tools/README.md, ffmpeg with hevc_nvenc,
and ffprobe. The original is read-only. A private partial output is renamed only after
every decoded stereo frame matches the pre-encode checksum. Existing outputs are refused.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import time
import uuid


def probe(path):
    return json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(path)]))


def pixel_hashes(path):
    with open(path, "rb") as source:
        for line in source:
            if line.strip() and not line.startswith(b"#"):
                yield line.rsplit(b",", 1)[1].strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--decoder", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--seconds", type=float, help="Prepare only a validation sample")
    parser.add_argument("--threads", type=int, default=12)
    args = parser.parse_args()
    if args.output.exists() or args.output.resolve() == args.input.resolve():
        parser.error("Refusing to overwrite an output or the MVC source")
    if args.seconds is not None and args.seconds <= 0:
        parser.error("--seconds must be positive")
    if args.work_dir.exists() and any(args.work_dir.iterdir()):
        parser.error("Use an empty work directory to preserve prior verification evidence")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    original = args.input.stat()
    metadata = probe(args.input)
    video = [s for s in metadata["streams"] if s["codec_type"] == "video"]
    if len(video) != 2 or any(s["codec_name"] != "h264" for s in video):
        parser.error("Expected a two-view Blu-ray AVC/MVC transport stream")
    # Codec names alone are not MVC evidence; require the explicit MVC input label.
    if ".mvc." not in args.input.name.lower():
        parser.error("Input must have a confirmed .MVC. label")
    base_pid, dependent_pid = [s["id"] for s in video]
    partial = args.output.with_name(".mvc-" + uuid.uuid4().hex + ".partial.mkv")
    source_hash = args.work_dir / "source.framemd5"
    encoded_hash = args.work_dir / "encoded.framemd5"
    children = []
    logs = []
    try:
        def launch(command, name, **kw):
            log = (args.work_dir / (name + ".log")).open("wb"); logs.append(log)
            child = subprocess.Popen(command, stderr=log, **kw); children.append(child); return child
        demux_args = [sys.executable, str(Path(__file__).with_name("mvc_demux.py")), str(args.input), "--base-pid", base_pid, "--dependent-pid", dependent_pid]
        if args.seconds: demux_args += ["--seconds", str(args.seconds)]
        demux = launch(demux_args, "demux", stdout=subprocess.PIPE)
        env = dict(os.environ, EDGE264MVC_THREADS=str(args.threads))
        decoder = launch([str(args.decoder.resolve()), "-", "-O"], "decoder", stdin=demux.stdout, stdout=subprocess.PIPE, env=env)
        demux.stdout.close()
        hasher = launch(["ffmpeg", "-v", "error", "-i", "pipe:0", "-map", "0:v:0", "-f", "framemd5", "-n", str(source_hash)], "source-hashes", stdin=subprocess.PIPE)
        command = ["ffmpeg", "-hide_banner", "-nostdin", "-loglevel", "warning", "-i", "pipe:0", "-i", str(args.input),
                   "-map", "0:v:0", "-map", "1:a:0?", "-map", "1:a?", "-map", "1:s?", "-map_metadata", "1",
                   "-c:v", "hevc_nvenc", "-preset", "p6", "-tune", "lossless", "-rc", "constqp", "-qp", "0", "-pix_fmt", "yuv420p",
                   "-c:a", "copy", "-c:a:0", "aac", "-b:a:0", "512k", "-disposition:a", "0", "-disposition:a:0", "default",
                   "-metadata:s:a:0", "title=AAC", "-c:s", "copy", "-metadata:s:v:0", "stereo_mode=left_right", "-fps_mode", "passthrough"]
        for key, option in [("color_primaries", "-color_primaries"), ("color_transfer", "-color_trc"), ("color_space", "-colorspace"), ("color_range", "-color_range")]:
            if video[0].get(key) not in (None, "unknown"): command += [option, video[0][key]]
        if args.seconds: command += ["-t", str(args.seconds)]
        command += ["-n", str(partial)]
        encoder = launch(command, "encoder", stdin=subprocess.PIPE)
        total = 0; last = time.monotonic()
        while data := decoder.stdout.read(1024 * 1024):
            hasher.stdin.write(data); encoder.stdin.write(data); total += len(data)
            if time.monotonic() - last > 15:
                print(f"Decoded stereo data: {total / (1024**3):.1f} GiB", flush=True); last = time.monotonic()
        decoder.stdout.close(); hasher.stdin.close(); encoder.stdin.close()
        for child in children:
            if child.wait() != 0: raise RuntimeError("MVC preparation stage failed; inspect the work-directory logs")
        print("Verifying every encoded stereo frame against the decoder output…", flush=True)
        subprocess.run(["ffmpeg", "-v", "error", "-i", str(partial), "-map", "0:v:0", "-f", "framemd5", "-n", str(encoded_hash)], check=True)
        from itertools import zip_longest
        count = 0; digest = hashlib.sha256()
        for source, encoded in zip_longest(pixel_hashes(source_hash), pixel_hashes(encoded_hash)):
            if source is None or source != encoded: raise RuntimeError("Encoded pixels or frame count changed")
            digest.update(source); count += 1
        if not count: raise RuntimeError("MVC decoder returned no stereo frames")
        demux_log = (args.work_dir / "demux.log").read_text()
        matched = re.search(r"Matched MVC access-unit pairs: (\d+)", demux_log)
        if not matched or int(matched.group(1)) != count:
            raise RuntimeError("Decoder frame count did not match the paired MVC access units")
        result = probe(partial)
        out_video = next(s for s in result["streams"] if s["codec_type"] == "video")
        if out_video["width"] != video[0]["width"] * 2 or out_video["height"] != video[0]["height"]:
            raise RuntimeError("Full-resolution eye dimensions were not preserved")
        original_audio = [s["codec_name"] for s in metadata["streams"] if s["codec_type"] == "audio"]
        output_audio = [s["codec_name"] for s in result["streams"] if s["codec_type"] == "audio"]
        if original_audio and output_audio != ["aac"] + original_audio: raise RuntimeError("Original audio tracks were not preserved")
        original_subtitles = [s["codec_name"] for s in metadata["streams"] if s["codec_type"] == "subtitle"]
        output_subtitles = [s["codec_name"] for s in result["streams"] if s["codec_type"] == "subtitle"]
        if original_subtitles != output_subtitles:
            raise RuntimeError("Original subtitle tracks were not preserved")
        after = args.input.stat()
        if (original.st_size, original.st_mtime_ns) != (after.st_size, after.st_mtime_ns): raise RuntimeError("Source changed during conversion")
        if args.output.exists(): raise RuntimeError("Output appeared during preparation; refusing to replace it")
        # Hard-link publication fails atomically if a destination appeared; never replace it.
        os.link(partial, args.output)
        partial.unlink()
        proof = {"frames":count,"pixel_hash_sha256":digest.hexdigest(),"width":out_video["width"],"height":out_video["height"],"source_size":original.st_size,"output_size":args.output.stat().st_size,"lossless":True,"audio":output_audio}
        (args.work_dir / "verification.json").write_text(json.dumps(proof, indent=2))
        print(json.dumps(proof), flush=True)
    finally:
        for child in children:
            if child.poll() is None: child.terminate()
        for child in children:
            try: child.wait(timeout=10)
            except subprocess.TimeoutExpired: child.kill(); child.wait()
        for log in logs: log.close()
        if partial.exists(): partial.unlink()


if __name__ == "__main__":
    main()
