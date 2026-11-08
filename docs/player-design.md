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
