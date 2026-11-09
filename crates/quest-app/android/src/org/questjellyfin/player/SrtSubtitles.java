package org.questjellyfin.player;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Minimal SubRip parser for external Jellyfin text subtitle streams. */
public final class SrtSubtitles {
    private static final Pattern TIME = Pattern.compile(
        "(\\d{1,2}):(\\d{2}):(\\d{2})[,.](\\d{1,3})\\s*-->\\s*(\\d{1,2}):(\\d{2}):(\\d{2})[,.](\\d{1,3})"
    );

    public static final class Cue {
        public final long startMs;
        public final long endMs;
        public final String text;
        public Cue(long startMs, long endMs, String text) {
            this.startMs = startMs;
            this.endMs = endMs;
            this.text = text;
        }
    }

    private SrtSubtitles() {}

    public static List<Cue> parse(String raw) {
        List<Cue> cues = new ArrayList<>();
        if (raw == null || raw.isEmpty()) return cues;
        String normalized = raw.replace("\uFEFF", "").replace("\r\n", "\n").replace('\r', '\n');
        String[] blocks = normalized.split("\n\\s*\n");
        for (String block : blocks) {
            String[] lines = block.split("\n");
            int timeLine = -1;
            for (int i = 0; i < lines.length; i++) {
                if (lines[i].contains("-->")) { timeLine = i; break; }
            }
            if (timeLine < 0) continue;
            Matcher matcher = TIME.matcher(lines[timeLine].trim());
            if (!matcher.find()) continue;
            long start = timestamp(matcher, 1);
            long end = timestamp(matcher, 5);
            if (end < start) continue;
            StringBuilder text = new StringBuilder();
            for (int i = timeLine + 1; i < lines.length; i++) {
                String line = stripTags(lines[i].trim());
                if (line.isEmpty()) continue;
                if (text.length() > 0) text.append('\n');
                text.append(line);
            }
            if (text.length() == 0) continue;
            cues.add(new Cue(start, end, text.toString()));
        }
        return cues;
    }

    public static String at(List<Cue> cues, int positionMs) {
        if (cues == null || cues.isEmpty() || positionMs < 0) return "";
        // Linear scan is fine for typical subtitle counts; cues are ordered.
        for (Cue cue : cues) {
            if (positionMs < cue.startMs) return "";
            if (positionMs <= cue.endMs) return cue.text;
        }
        return "";
    }

    /** True for codecs the immersive player can show as text (not bitmap). */
    public static boolean isTextCodec(String codec) {
        if (codec == null) return false;
        String value = codec.trim().toLowerCase(java.util.Locale.ROOT);
        return value.equals("subrip") || value.equals("srt") || value.equals("webvtt")
            || value.equals("vtt") || value.equals("mov_text") || value.equals("tx3g")
            || value.equals("text") || value.equals("ass") || value.equals("ssa");
    }

    private static long timestamp(Matcher matcher, int group) {
        int h = Integer.parseInt(matcher.group(group));
        int m = Integer.parseInt(matcher.group(group + 1));
        int s = Integer.parseInt(matcher.group(group + 2));
        String frac = matcher.group(group + 3);
        while (frac.length() < 3) frac = frac + "0";
        if (frac.length() > 3) frac = frac.substring(0, 3);
        return (((h * 60L + m) * 60L) + s) * 1000L + Integer.parseInt(frac);
    }

    private static String stripTags(String line) {
        // Drop basic HTML / ASS override tags; keep readable dialogue.
        String plain = line.replaceAll("(?i)<br\\s*/?>", "\n");
        plain = plain.replaceAll("<[^>]+>", "");
        plain = plain.replaceAll("\\{\\\\[^}]*\\}", "");
        return plain.trim();
    }
}
