package org.questjellyfin.player;

/** Cycle Off (-1) then each MediaPlayer subtitle / timed-text track. */
public final class SubtitleTrackSelector {
    private SubtitleTrackSelector() {}

    /** Next choice after {@code selected}; empty track lists stay Off. */
    public static int next(int[] tracks, int selected) {
        if (tracks == null || tracks.length == 0) return -1;
        if (selected < 0) return tracks[0];
        for (int i = 0; i < tracks.length; i++) {
            if (tracks[i] == selected) return i + 1 < tracks.length ? tracks[i + 1] : -1;
        }
        return tracks[0];
    }

    /**
     * Index of the text subtitle stream immersive playback should offer.
     * Bitmap codecs are skipped. The first default, forced, or external text stream wins;
     * otherwise the first text stream. {@code -1} when there is no text stream.
     * {@code takeNow[i]} is true for default, forced, or external.
     */
    public static int preferredText(String[] codecs, boolean[] takeNow) {
        int fallback = -1;
        int count = codecs == null ? 0 : codecs.length;
        for (int i = 0; i < count; i++) {
            if (!SrtSubtitles.isTextCodec(codecs[i])) continue;
            boolean take = takeNow != null && i < takeNow.length && takeNow[i];
            if (fallback < 0) fallback = i;
            if (take) return i;
        }
        return fallback;
    }

    /** Label for the transport button. */
    public static String label(int[] tracks, int selected, String language) {
        if (tracks == null || tracks.length == 0) return "No subs";
        if (selected < 0) return "Subs off";
        if (language != null && !language.isEmpty() && !"und".equalsIgnoreCase(language)) {
            return "Subs " + language;
        }
        return "Subs on";
    }
}
