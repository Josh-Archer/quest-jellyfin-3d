package org.questjellyfin.player;
public final class PlaybackGateTest {
    private static void check(boolean value) { if (!value) throw new AssertionError(); }
    public static void main(String[] args) {
        PlaybackGate gate = new PlaybackGate();
        check(!gate.canPlay()); gate.focus(false); check(!gate.canPlay());
        gate.focus(true); check(gate.canPlay());
        gate.focus(false); check(!gate.canPlay());
        gate.toggle(); gate.focus(true); check(!gate.canPlay());
        gate.toggle(); check(gate.canPlay());
        gate.interrupt(); check(!gate.canPlay()); gate.focus(true); check(!gate.canPlay());
        gate.toggle(); check(gate.canPlay());
        gate.toggle(); gate.focus(false); gate.focus(true); check(!gate.canPlay());
        gate.toggle(); check(gate.canPlay());
        System.out.println("PlaybackGate: startup, removal, manual pause and explicit resume passed");
    }
}
