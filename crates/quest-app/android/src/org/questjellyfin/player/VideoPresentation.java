package org.questjellyfin.player;

/** Prevent stale stereo imagery during initial buffering, rebuffering and seeks. */
final class VideoPresentation {
    private boolean frame;
    private boolean buffering;
    private boolean seeking;
    private boolean stalled;
    private int position = -1;
    private long movedAt;
    void firstFrame() { frame = true; }
    void buffering(boolean value) { buffering = value; }
    void seeking(boolean value) { seeking = value; stalled = false; position = -1; }
    void observePosition(int value, boolean playing, long now) {
        if (value != position || !playing) { position = value; movedAt = now; stalled = false; }
        else if (frame && now - movedAt >= 1500) stalled = true;
    }
    boolean waiting() { return buffering || seeking || stalled; }
    String diagnostics() { return "frame="+frame+" buffering="+buffering+" seeking="+seeking+" stalled="+stalled; }
    int state() { return waiting() ? 2 : frame ? 1 : 0; }
}
