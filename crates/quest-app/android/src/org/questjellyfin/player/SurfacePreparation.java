package org.questjellyfin.player;

/** Decide whether the prepared decoder needs a surface matching its output dimensions. */
final class SurfacePreparation {
    private SurfacePreparation() {}
    static boolean needsResize(int expectedWidth, int expectedHeight, int actualWidth, int actualHeight) {
        return actualWidth > 0 && actualHeight > 0 &&
            (actualWidth != expectedWidth || actualHeight != expectedHeight);
    }
}
