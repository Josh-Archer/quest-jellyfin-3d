import java.util.List;
import org.questjellyfin.player.SrtSubtitles;
public final class SrtSubtitlesTest {
    private static void check(boolean ok, String message) {
        if (!ok) throw new AssertionError(message);
    }
    public static void main(String[] args) {
        check(SrtSubtitles.isTextCodec("subrip"), "subrip");
        check(SrtSubtitles.isTextCodec("ASS"), "ass");
        check(!SrtSubtitles.isTextCodec("pgssub"), "bitmap");
        check(!SrtSubtitles.isTextCodec(null), "null codec");
        String srt = "1\n00:00:01,000 --> 00:00:02,500\nHello <i>world</i>\n\n"
            + "2\n00:00:03,000 --> 00:00:04,000\nLine A\nLine B\n\n"
            + "3\n00:01:00.000 --> 00:01:01.000\n{\\an8}Later\n";
        List<SrtSubtitles.Cue> cues = SrtSubtitles.parse(srt);
        check(cues.size() == 3, "cue count " + cues.size());
        check(cues.get(0).startMs == 1000 && cues.get(0).endMs == 2500, "first timing");
        check("Hello world".equals(cues.get(0).text), "strip tags: " + cues.get(0).text);
        check("Line A\nLine B".equals(cues.get(1).text), "multiline");
        check(cues.get(2).startMs == 60000, "dot millis");
        check("Later".equals(cues.get(2).text), "ass override stripped: " + cues.get(2).text);
        check("".equals(SrtSubtitles.at(cues, 0)), "before first");
        check("Hello world".equals(SrtSubtitles.at(cues, 1500)), "inside first");
        check("".equals(SrtSubtitles.at(cues, 2600)), "gap");
        check("Line A\nLine B".equals(SrtSubtitles.at(cues, 3000)), "second");
        check("".equals(SrtSubtitles.at(cues, 120000)), "after last");
        check(SrtSubtitles.parse("").isEmpty(), "empty");
        check(SrtSubtitles.parse(null).isEmpty(), "null");
    }
}
