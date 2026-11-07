package org.questjellyfin.player;

/** Monotonic idle policy for panel playback chrome. Pause/loading always exposes navigation. */
final class PlaybackChrome {
    static final long HIDE_AFTER_MS = 5000;
    private long lastInteraction;
    void interact(long now) { lastInteraction = now; }
    boolean visible(long now, boolean playing, boolean interacting) {
        if (!playing || interacting) lastInteraction = now;
        return !playing || interacting || now - lastInteraction < HIDE_AFTER_MS;
    }
}
