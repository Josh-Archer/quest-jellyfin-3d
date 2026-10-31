package org.questjellyfin.player;

/** Startup waits for XR focus. Focus loss requires an explicit Play after returning. */
final class PlaybackGate {
    private boolean focused;
    private boolean wantsPlay = true;
    private boolean everFocused;
    void focus(boolean value) {
        if (!value && everFocused) wantsPlay = false;
        focused = value;
        everFocused |= value;
    }
    boolean canPlay() { return focused && wantsPlay; }
    void toggle() { if (focused) wantsPlay = !wantsPlay; }
    void interrupt() { if (everFocused) wantsPlay = false; }
}
