package org.questjellyfin.player;

import android.media.MediaCodecInfo;
import android.media.MediaCodecList;
import android.media.MediaFormat;

/** Query the regular hardware decoder, never the lower-throughput low-latency variant. */
final class VideoDecoderCapabilities {
    private static Boolean hevc8k60;
    static synchronized boolean supportsHevc8k60() {
        if (hevc8k60 != null) return hevc8k60;
        hevc8k60 = false;
        try {
            MediaFormat format = MediaFormat.createVideoFormat("video/hevc", 7680, 4320);
            format.setInteger(MediaFormat.KEY_FRAME_RATE, 60);
            for (MediaCodecInfo codec : new MediaCodecList(MediaCodecList.REGULAR_CODECS).getCodecInfos()) {
                if (codec.isEncoder() || !codec.isHardwareAccelerated() || codec.getName().contains("low_latency") || codec.getName().contains("secure")) continue;
                for (String type : codec.getSupportedTypes()) {
                    if (!type.equals("video/hevc")) continue;
                    MediaCodecInfo.CodecCapabilities caps = codec.getCapabilitiesForType(type);
                    if (caps.isFormatSupported(format) && caps.getVideoCapabilities().areSizeAndRateSupported(7680, 4320, 60)) {
                        hevc8k60 = true; return true;
                    }
                }
            }
        } catch (RuntimeException ignored) { /* Preserve the conservative profile on failed queries. */ }
        return false;
    }
}
