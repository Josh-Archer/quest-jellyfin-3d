package org.questjellyfin.player;

public class PlaybackChromeTest {
    private static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    public static void main(String[] args) {
        PlaybackChrome chrome = new PlaybackChrome();
        check(chrome.visible(10000, false, false)); // loading/prepared but not playing
        check(chrome.visible(14999, true, false));
        check(!chrome.visible(15000, true, false));
        chrome.interact(16000); // movie tap/controller input reveals the bar
        check(chrome.visible(16000, true, false));
        check(chrome.visible(23000, true, true)); // seeking/hovering over chrome keeps it usable
        check(chrome.visible(27999, true, false));
        check(!chrome.visible(28000, true, false));
        check(chrome.visible(28001, false, false)); // pause/buffering/focus interruption
        check(chrome.visible(33000, true, false));
        check(!chrome.visible(33001, true, false));
        System.out.println("PlaybackChrome: idle, interaction, loading, pause and resume passed");
    }
}
