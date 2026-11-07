"""Synthetic transport fixtures exercise both-eye pairing and corruption rejection."""
import io
import unittest
from mvc_demux import demux, pes_payload
from prepare_mvc import pixel_hashes
import tempfile
from pathlib import Path


def stamp(value):
    return bytes([0x21 | ((value >> 29) & 14), (value >> 22) & 255,
                  ((value >> 14) & 254) | 1, (value >> 7) & 255, ((value << 1) & 254) | 1])


def packet(pid, counter, pts, payload, m2ts=True):
    pes = b'\0\0\1\xe0' + (8 + len(payload)).to_bytes(2, 'big') + b'\x80\x80\x05' + stamp(pts) + payload
    adaptation = 183 - len(pes)
    ts = bytes([0x47, 0x40 | (pid >> 8), pid & 255, 0x30 | counter, adaptation])
    ts += bytes(adaptation) + pes
    assert len(ts) == 188
    return (bytes(4) if m2ts else b'') + ts


def fixture(m2ts=True):
    return b''.join(packet(pid, i, i * 3750, label + bytes([i]), m2ts)
                    for i in range(3) for pid, label in [(0x1011, b'L'), (0x1012, b'R')])


class DemuxTest(unittest.TestCase):
    def test_both_transport_formats_preserve_eye_order_and_all_frames(self):
        for m2ts in [False, True]:
            out = io.BytesIO()
            self.assertEqual(demux(io.BytesIO(fixture(m2ts)), out, 0x1011, 0x1012), 3)
            self.assertEqual(out.getvalue(), b'L\0R\0L\1R\1L\2R\2')

    def test_sample_is_bounded_at_pair_boundary(self):
        out = io.BytesIO()
        self.assertEqual(demux(io.BytesIO(fixture()), out, 0x1011, 0x1012, .04), 1)
        self.assertEqual(out.getvalue(), b'L\0R\0')

    def test_missing_eye_is_rejected(self):
        data = fixture()[:-192]
        with self.assertRaisesRegex(ValueError, 'Unpaired'):
            demux(io.BytesIO(data), io.BytesIO(), 0x1011, 0x1012)

    def test_disagreeing_eye_timestamps_are_rejected(self):
        data = packet(0x1011, 0, 0, b'L') + packet(0x1012, 0, 1, b'R')
        with self.assertRaises(ValueError):
            demux(io.BytesIO(data), io.BytesIO(), 0x1011, 0x1012)

    def test_continuity_and_truncation_fail_closed(self):
        data = bytearray(fixture()); data[2 * 192 + 7] ^= 1
        for broken in [data, fixture()[:-1]]:
            with self.assertRaises(ValueError):
                demux(io.BytesIO(broken), io.BytesIO(), 0x1011, 0x1012)

    def test_duplicate_pids_and_missing_pes_timestamp_are_rejected(self):
        with self.assertRaises(ValueError):
            demux(io.BytesIO(fixture()), io.BytesIO(), 0x1011, 0x1011)
        with self.assertRaises(ValueError):
            pes_payload(b'\0' * 20)

    def test_frame_verification_ignores_container_timebase_but_reads_every_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'frames'
            path.write_bytes(b'#tb 0: 1/24\n0,0,0,1,100, abc\n0,1,1,1,100, def\n')
            self.assertEqual(list(pixel_hashes(path)), [b'abc', b'def'])


if __name__ == '__main__':
    unittest.main()
