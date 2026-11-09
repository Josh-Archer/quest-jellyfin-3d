import java.util.List;
import org.questjellyfin.player.SrtSubtitles;
import org.questjellyfin.player.SubtitleTrackSelector;

/** Wires track choice and cue lookup the way cinema playback does. */
public final class SubtitlePlaybackIntegrationTest {
    public static void main(String[] args) {
        String srt = "1\n00:00:01,000 --> 00:00:02,500\nHello world\n\n"
            + "2\n00:00:03,000 --> 00:00:04,000\n<i>Line A</i>\nLine B\n";
        List<SrtSubtitles.Cue> cues = SrtSubtitles.parse(srt);
        int selected = -1;
        check("".equals(shown(cues, selected, 1500)), "off hides a cue that is on screen");

        int[] tracks = new int[] {2, 5};
        selected = SubtitleTrackSelector.next(tracks, selected);
        check(selected == 2, "first Subs press enables the first track");
        check("Subs eng".equals(SubtitleTrackSelector.label(tracks, selected, "eng")), "label");
        check("Hello world".equals(shown(cues, selected, 1500)), "cue at 1.5s");
        check("".equals(shown(cues, selected, 2600)), "gap stays blank");
        check("Line A\nLine B".equals(shown(cues, selected, 3500)), "tags stripped");

        selected = SubtitleTrackSelector.next(tracks, selected);
        check(selected == 5, "second press advances");
        selected = SubtitleTrackSelector.next(tracks, selected);
        check(selected == -1, "third press returns to Off");
        check("".equals(shown(cues, selected, 1500)), "off again");
        check("Subs off".equals(SubtitleTrackSelector.label(tracks, selected, "eng")), "off label");

        int bitmapOnly = SubtitleTrackSelector.preferredText(
            new String[] {"pgssub", "hdmv_pgs_subtitle"}, new boolean[] {true, true});
        check(bitmapOnly == -1, "bitmap subs are not a text track");
        int forced = SubtitleTrackSelector.preferredText(
            new String[] {"pgssub", "subrip", "ass"},
            new boolean[] {true, false, true});
        check(forced == 2, "forced text wins over an earlier plain text stream");
        int firstText = SubtitleTrackSelector.preferredText(
            new String[] {"pgssub", "webvtt"}, new boolean[] {true, false});
        check(firstText == 1, "otherwise the first text stream");
        System.out.println("SubtitlePlaybackIntegrationTest passed");
    }

    /** Cinema shows a cue only while a subtitle track is selected. */
    private static String shown(List<SrtSubtitles.Cue> cues, int selected, int positionMs) {
        if (selected < 0) return "";
        return SrtSubtitles.at(cues, positionMs);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
