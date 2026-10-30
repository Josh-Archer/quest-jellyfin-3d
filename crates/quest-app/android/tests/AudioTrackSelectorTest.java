import org.questjellyfin.player.AudioTrackSelector;
public final class AudioTrackSelectorTest {
    private static void check(int actual, int expected) {
        if (actual != expected) throw new AssertionError("expected " + expected + ", got " + actual);
    }
    public static void main(String[] args) {
        check(AudioTrackSelector.next(new int[]{}, -1), -1);
        check(AudioTrackSelector.next(new int[]{2}, -1), 2);
        check(AudioTrackSelector.next(new int[]{2}, 2), 2);
        check(AudioTrackSelector.next(new int[]{1, 4, 8}, 1), 4);
        check(AudioTrackSelector.next(new int[]{1, 4, 8}, 4), 8);
        check(AudioTrackSelector.next(new int[]{1, 4, 8}, 8), 1);
        check(AudioTrackSelector.next(new int[]{1, 4, 8}, 3), 4);
    }
}
