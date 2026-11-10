# Player design and comfort

The native OpenXR player adapts the Meta Horizon OS design system to its existing
movie-attached renderer. It does not use Jetpack Compose UI Set components.

Sources:
- [Typography](https://developers.meta.com/vr/design/styles_typography/): bundled Inter, semantic type hierarchy, 18px body controls and 16px secondary labels.
- [Hit targets](https://developers.meta.com/vr/design/styles_inputs_hit_targets/): 60dp primary/hand targets, 48dp timeline hit region, inert gaps between actions.
- [Colors](https://developers.meta.com/vr/design/styles_color/): semantic surface/text/action tokens, soft gray surfaces and text, contrast and a restrained accent.
- [Buttons](https://developers.meta.com/vr/design/buttons/): circular primary transport, pill-shaped supporting controls, clear selected and hover states.
- [Button states](https://developers.meta.com/vr/design/buttons_specs/): distinct hover and pressed feedback.

Controls use a 768dp layout rendered at 2x into their own 1536×784 swapchain.
Room materials and manipulation handles retain the shared DeskQuest HUD atlas.
The controls remain attached to flat and curved movies. In spherical VR they use
a compact, movable workspace rather than the bottom of an imaginary large square.
View settings include projection, tilt, recenter, reversible eye order, and a
single-eye 2D option. 2D uses one complete cropped eye for both eyes, not the whole
packed stereo texture.

During preparation, buffering, seeks and detected playback stalls the movie's
surface remains submitted behind an opaque neutral cover, keeping the decoder's
producer queue draining. Both eye views are hidden; binocular controls and the
loading state remain available. Buffer completion before the first submitted layer is handled;
seeking listeners remain active after resume and repeated transport actions.

The regular non-secure hardware decoder is queried on the headset. When it reports
7680×4320 HEVC at 60fps, Jellyfin receives that measured tier; otherwise the previous
conservative profile remains. AV1 and wider 8192-pixel sources keep their existing
limits. User-selected bitrate and transcoding preferences continue to apply.
Device capability reporting is not a throughput guarantee; real playback is still
validated on the headset and server. No source resolution is silently reduced as
a buffering workaround.

Immersive Direct Play uses Android's public `MediaDataSource` API with an
HTTP byte-range reader. Its LRU retains up to 128 MiB in 8 MiB pages, allowing
interleaved audio/video reads and resumed seeks to reuse separate file ranges.
Successful HTTP connections remain reusable. TLS verification stays enabled,
authorization remains in headers, redirects and compressed/mismatched/truncated
ranges are rejected, and closing the source cancels an outstanding read. HLS and
server conversion keep the platform URL source. This does not re-encode media.

VR180/VR360 packed stereo also has per-title horizontal alignment under View.
Align + / Align − adjust relative eye orientation by 0.5°; holding either button
repeats after an initial delay. The bounded range is −8° to +8°, and Reset returns
to the original zero correction. Eye crops, source resolution, panorama coverage,
and headset IPD are unchanged. The single-eye 2D view bypasses this correction.
This is a manual viewing adjustment for recorded disparity, not a repair of the
recording's stereo depth. Confirm packing, projection and eye order first.

## MV-HEVC and other multiview codecs

Quest's regular hardware decoder and the current `xr-video` MediaCodec path expose a
single HEVC/AV1 surface. They do not decode independent MV-HEVC (or Blu-ray MVC)
dependent views, so cinema cannot honestly submit both eyes from a raw multiview
bitstream. Ordinary HEVC capability reporting does not imply multiview decoding.

Preparation therefore:

1. Detects `MvHevc` / `Mvc` from Jellyfin `Video3DFormat`, codec/profile labels, or
   filename tokens.
2. Prefers a packed SBS/OU alternate already indexed on the same Jellyfin item when
   one exists, and plays that alternate through the existing packed-stereo crop path.
3. Keeps raw MV-HEVC out of the cinema compositor. When no packed alternate is
   offered, `prepare` fails with an error that names the missing dual-view MV-HEVC
   decode path rather than relabeling the stream as mono HEVC or SBS.

Server-side packed alternates (and future dual-view decode work) remain the path to
spatial video; this client does not pretend MediaCodec already provides one.

## Fisheye projection

OpenXR provides equirectangular composition layers (`XR_KHR_composition_layer_equirect2`),
not a fisheye/polar lens layer. There is no app-side video remesh on the MediaCodec
surface path, so fisheye cannot be sampled with a true circular lens map today.

Cinema therefore submits filename- or tag-classified `Fisheye` items through the same
`CompositionLayerEquirect2KHR` path used for VR180, with hemispherical
(`central_horizontal_angle = π`, full vertical ±π/2) coverage. That is a FOV and
placement approximation only: it does not convert dual-fisheye circles into
longitude/latitude. Raw circular fisheye frames will look wrong until a mesh or
shader remap exists. Prefer remapped equirectangular sources (`Equirectangular180` /
`Equirectangular360`) when available. Fisheye is never inferred from resolution or
aspect alone; ordinary flat movies stay flat unless the basename or Jellyfin tags
say otherwise.
