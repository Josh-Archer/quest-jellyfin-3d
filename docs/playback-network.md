# Diagnosing high-bitrate playback on Quest

Direct Play sends the original media to the headset. Jellyfin's server GPU can be
idle while Quest's hardware video decoder is active. A server transcode instead
uses the encoder configured in Jellyfin; confirm its actual playback session and
FFmpeg command before changing hardware acceleration settings.

For a connected Quest, `adb shell dumpsys media.player` identifies the active
video decoder, dimensions, decoded frame count and dropped frames. The regular
Qualcomm HEVC decoder is `c2.qti.hevc.decoder`. Hardware capability and first-frame
success do not establish sustained network throughput.

Check the actual route from the headset, rather than the development computer:

```sh
adb shell cmd wifi status
adb shell ping -c 2 your-jellyfin-hostname
adb shell settings get global private_dns_mode
adb shell dumpsys connectivity
```

A strong Wi-Fi link does not establish the speed of a public reverse proxy or
Internet route. Compare authenticated range downloads of the same media through
the public and local routes, retaining HTTPS hostname and certificate validation.
Do not put Jellyfin credentials in command arguments, logs, screenshots or commits.
The stream needs sustained throughput above its bitrate, including bitrate peaks.

Android's automatic Private DNS can select an external encrypted DNS resolver
when the local split-DNS resolver does not offer DNS over TLS. The Jellyfin hostname
can then resolve publicly even on home Wi-Fi. Check `UsePrivateDns`, the validated
resolver addresses and the resolved server address in the device diagnostics.
Use a local DNS configuration that resolves the same HTTPS hostname to the LAN
server. Turning Private DNS off on the headset is one device-specific workaround;
it changes DNS behavior for all apps, so do not apply it automatically in the app.
Record the previous setting before changing it, and restore that setting if the
workaround is no longer needed. Reopen playback to replace existing connections.

Preserve the selected quality while diagnosing the route. A lower-resolution
transcode can conceal a throughput problem while degrading a stereo panorama.
