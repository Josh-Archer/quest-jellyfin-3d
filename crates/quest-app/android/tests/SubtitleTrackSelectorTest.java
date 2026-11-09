import org.questjellyfin.player.SubtitleTrackSelector;
public final class SubtitleTrackSelectorTest {
    private static void check(int actual, int expected) {
        if (actual != expected) throw new AssertionError("expected " + expected + ", got " + actual);
    }
    private static void check(String actual, String expected) {
        if (!expected.equals(actual)) throw new AssertionError("expected " + expected + ", got " + actual);
    }
    public static void main(String[] args) {
        check(SubtitleTrackSelector.next(new int[]{}, -1), -1);
        check(SubtitleTrackSelector.next(new int[]{2}, -1), 2);
        check(SubtitleTrackSelector.next(new int[]{2}, 2), -1);
        check(SubtitleTrackSelector.next(new int[]{1, 4, 8}, -1), 1);
        check(SubtitleTrackSelector.next(new int[]{1, 4, 8}, 1), 4);
        check(SubtitleTrackSelector.next(new int[]{1, 4, 8}, 4), 8);
        check(SubtitleTrackSelector.next(new int[]{1, 4, 8}, 8), -1);
        check(SubtitleTrackSelector.next(new int[]{1, 4, 8}, 3), 1);
        check(SubtitleTrackSelector.label(new int[]{}, -1, null), "No subs");
        check(SubtitleTrackSelector.label(new int[]{1}, -1, "eng"), "Subs off");
        check(SubtitleTrackSelector.label(new int[]{1}, 1, "eng"), "Subs eng");
        check(SubtitleTrackSelector.label(new int[]{1}, 1, "und"), "Subs on");
        check(SubtitleTrackSelector.label(new int[]{1}, 1, null), "Subs on");
    }
}
