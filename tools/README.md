# MVC preparation

Quest does not natively decode Blu-ray MVC. `prepare_mvc.py` creates a separate
full-resolution full-side-by-side HEVC version by decoding **both** AVC/MVC views.
It never overwrites the source or an existing destination. FFmpeg alone normally
exposes only the AVC base view and is not an MVC-to-stereo converter.

Requirements: Python 3.9+, FFmpeg/ffprobe with `hevc_nvenc`, a supported NVIDIA GPU,
and the BSD-licensed [edge264-mvc](https://github.com/jens-duttke/edge264-mvc) decoder.
Build the tested decoder revision in a scratch directory:

```sh
git clone https://github.com/jens-duttke/edge264-mvc.git
cd edge264-mvc
git checkout 515a7ba9afd945387d41c118404c2ee9dbaf6b83
make -j8 VARIANTS= BUILDTEST=yes
```

Use a new work directory for each run. First prepare a short validation sample
outside the library, then run the complete conversion without `--seconds`:

```sh
python3 tools/prepare_mvc.py '/media/Movie (2013).3D.MVC.m2ts' \
  '/media/Movie (2013) - 3D.FSBS.mkv' \
  --decoder /scratch/edge264-mvc/edge264mvc_test \
  --work-dir /scratch/movie-verification --seconds 12
```

The input must be a confirmed `.MVC.` transport stream with exactly two H.264
video PIDs. The demuxer checks transport continuity and pairs matching DTS/PTS
before forwarding both views to the decoder. Missing views fail the conversion.
The HEVC output retains the original resolution of each eye. Every output frame
is decoded again and its pixel checksum compared to the uncompressed stereo
source before publication. `verification.json` records frame count and proof.
Original audio and subtitles are copied; an AAC compatibility track is added.

Lossless video is large and can require hundreds of gigabytes. Check storage
and network capacity first. Lossless preparation does not guarantee lossless
headset playback: Jellyfin may transcode to fit hardware or bandwidth settings.
The tool does not modify Jellyfin credentials or database. Refresh the library
and group the output with the original as alternate versions in Jellyfin. The
app prefers explicitly packed versions of an MVC item, including when Jellyfin
must transcode that version. If no alternate exists, it keeps MVC marked as
unsupported for immersive stereo rather than pretending the base view is 3D.

Run transport and verification tests with:

```sh
python3 -m unittest discover -s tools -p 'test_*.py'
```
