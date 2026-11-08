#!/usr/bin/env python3
"""Join matched Blu-ray AVC/MVC PES access units into a two-view Annex B stream.

The two PIDs are explicit, never guessed from AVC codec labels. Matched DTS and PTS,
transport continuity and the presence of both views are required. Output is binary stdout.
"""
import argparse
import re
import sys


def timestamp(data):
    if len(data) != 5 or any(not data[i] & 1 for i in (0, 2, 4)):
        raise ValueError("Invalid PES timestamp")
    return ((data[0] >> 1 & 7) << 30) | (data[1] << 22) | ((data[2] >> 1) << 15) | (data[3] << 7) | (data[4] >> 1)


def pes_payload(data):
    if len(data) < 14 or data[:3] != b"\x00\x00\x01" or not data[7] & 128:
        raise ValueError("Video PES omitted its timestamp or header")
    pts = timestamp(data[9:14])
    dts = timestamp(data[14:19]) if data[7] & 64 else pts
    start = 9 + data[8]
    size = int.from_bytes(data[4:6], "big")
    end = 6 + size if size else len(data)
    if start > end or end > len(data):
        raise ValueError("Truncated video PES")
    return pts, dts, bytes(data[start:end])



def write_views(output, base, dependent):
    # Blu-ray can terminate both PES views with end-of-sequence NALs. A base
    # marker must not drain the decoder before the dependent view is supplied.
    endings = []
    for payload in (base, dependent):
        starts = list(re.finditer(rb"\x00\x00(?:\x00)?\x01", payload))
        if not starts:
            output.write(payload)
            continue
        output.write(payload[:starts[0].start()])
        for i, start in enumerate(starts):
            end = starts[i + 1].start() if i + 1 < len(starts) else len(payload)
            nal = payload[start.start():end]
            if start.end() < end and payload[start.end()] & 31 in (10, 11):
                endings.append(nal)
            else:
                output.write(nal)
    for nal in endings:
        output.write(nal)

def demux(source, output, base_pid, dependent_pid, seconds=None):
    if base_pid == dependent_pid:
        raise ValueError("MVC needs two distinct view PIDs")
    buffers, counters, ready = {}, {}, {}
    first_dts, last_dts, pairs = None, None, 0
    prefix = source.read(392)
    if len(prefix) < 192:
        raise ValueError("Empty or truncated transport stream")
    if len(prefix) > 196 and prefix[4] == 0x47 and prefix[196] == 0x47:
        packet_size, offset = 192, 4
    elif prefix[0] == 0x47 and prefix[188] == 0x47:
        packet_size, offset = 188, 0
    else:
        raise ValueError("Expected MPEG-TS or Blu-ray M2TS packets")
    pending = prefix

    def flush(pid):
        if pid not in buffers:
            return
        pts, dts, payload = pes_payload(buffers.pop(pid))
        entry = ready.setdefault(dts, {})
        if pid in entry:
            raise ValueError("Duplicate view timestamp")
        entry[pid] = (pts, payload)

    while True:
        chunk = source.read(packet_size * 8192)
        pending += chunk
        count = len(pending) // packet_size
        for n in range(count):
            packet = pending[n * packet_size + offset:(n + 1) * packet_size]
            if packet[0] != 0x47 or packet[1] & 128 or packet[3] & 192:
                raise ValueError("Corrupt or scrambled transport packet")
            pid = ((packet[1] & 31) << 8) | packet[2]
            if pid not in (base_pid, dependent_pid) or not packet[3] & 16:
                continue
            counter = packet[3] & 15
            if pid in counters and counter != (counters[pid] + 1) % 16:
                raise ValueError("Video transport continuity error")
            counters[pid] = counter
            start = 4 + (1 + packet[4] if packet[3] & 32 else 0)
            if start >= 188:
                raise ValueError("Invalid transport adaptation field")
            if packet[1] & 64:
                flush(pid)
                buffers[pid] = bytearray()
            if pid in buffers:
                buffers[pid].extend(packet[start:])
                if len(buffers[pid]) > 64 * 1024 * 1024:
                    raise ValueError("Video PES exceeded the bounded buffer")
            while ready:
                dts = min(ready)
                if len(ready[dts]) != 2:
                    break
                views = ready.pop(dts)
                if views[base_pid][0] != views[dependent_pid][0]:
                    raise ValueError("MVC eye presentation timestamps disagree")
                if last_dts is not None and dts <= last_dts:
                    raise ValueError("MVC decode timestamps are not ordered")
                if first_dts is None:
                    first_dts = dts
                if seconds is not None and dts - first_dts >= seconds * 90000:
                    return pairs
                write_views(output, views[base_pid][1], views[dependent_pid][1])
                last_dts = dts
                pairs += 1
            if len(ready) > 64:
                raise ValueError("Missing MVC eye; refusing a one-view conversion")
        pending = pending[count * packet_size:]
        if not chunk:
            break
    if pending:
        raise ValueError("Truncated transport packet")
    for pid in list(buffers):
        flush(pid)
    for dts in sorted(ready):
        views = ready[dts]
        if len(views) != 2 or views[base_pid][0] != views[dependent_pid][0]:
            raise ValueError("Unpaired MVC eye at end of stream")
        if last_dts is not None and dts <= last_dts:
            raise ValueError("MVC decode timestamps are not ordered")
        write_views(output, views[base_pid][1], views[dependent_pid][1])
        last_dts = dts
        pairs += 1
    if not pairs:
        raise ValueError("No paired AVC/MVC video found")
    return pairs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    parser.add_argument("--base-pid", type=lambda s: int(s, 0), required=True)
    parser.add_argument("--dependent-pid", type=lambda s: int(s, 0), required=True)
    parser.add_argument("--seconds", type=float)
    args = parser.parse_args()
    if args.seconds is not None and args.seconds <= 0:
        parser.error("--seconds must be positive")
    try:
        with open(args.input, "rb") as source:
            count = demux(source, sys.stdout.buffer, args.base_pid, args.dependent_pid, args.seconds)
        print(f"Matched MVC access-unit pairs: {count}", file=sys.stderr)
    except (OSError, ValueError) as error:
        parser.exit(1, f"MVC demux failed: {error}\n")


if __name__ == "__main__":
    main()
