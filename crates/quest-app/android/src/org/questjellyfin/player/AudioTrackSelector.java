package org.questjellyfin.player;

/** Select the next available audio track in media-player order. */
public final class AudioTrackSelector {
    private AudioTrackSelector() {}
    public static int next(int[] audioTracks, int selected) {
        if (audioTracks.length == 0) return -1;
        for (int track : audioTracks) if (track > selected) return track;
        return audioTracks[0];
    }
}
