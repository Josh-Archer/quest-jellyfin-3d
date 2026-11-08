package org.questjellyfin.player;

import android.content.res.Resources;
import android.graphics.Typeface;

/** Semantic native-renderer theme following Meta Horizon OS color, type and hit-target guidance.
 * See docs/player-design.md. Inter is bundled under the SIL Open Font License.
 */
final class HorizonStyle {
    static final int WIDTH = 768, MAIN_HEIGHT = 200, TOOLS_OFFSET = 192, TOOLS_HEIGHT = 392, SCALE = 2;
    static final int SURFACE = 0xFA202124, SURFACE_RAISED = 0xFF34363B;
    static final int TEXT = 0xFFDADADA, TEXT_SECONDARY = 0xFFADB3BD;
    static final int PRIMARY = 0xFFDADADA, ON_PRIMARY = 0xFF202124;
    static final int HOVER = 0xFF50545C, PRESSED = 0xFF626973, SELECTED = 0xFF244F78;
    static final int ACCENT = 0xFF8ABCF4, OUTLINE = 0xFF707782;
    static Typeface regular = Typeface.create("sans-serif", Typeface.NORMAL);
    static Typeface medium = Typeface.create("sans-serif-medium", Typeface.NORMAL);
    static void initialize(Resources resources, String packageName) {
        int font = resources.getIdentifier("inter", "font", packageName);
        if (font != 0) { regular = resources.getFont(font); medium = Typeface.create(regular, 600, false); }
    }
    private HorizonStyle() {}
}
