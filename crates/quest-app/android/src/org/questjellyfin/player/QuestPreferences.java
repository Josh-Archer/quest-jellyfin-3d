package org.questjellyfin.player;

/** Shared bounds and defaults used by the settings screen and immersive activity. */
final class QuestPreferences {
    static final float DEFAULT_WIDTH = 4f;
    static final float DEFAULT_DISTANCE = 2.2f;
    static float screenWidth(float value) { return finiteClamp(value, 1.5f, 6f, DEFAULT_WIDTH); }
    static float screenDistance(float value) { return finiteClamp(value, 1f, 5f, DEFAULT_DISTANCE); }
    static int columns(int value) { return Math.max(3, Math.min(6, value)); }
    private static float finiteClamp(float value, float min, float max, float fallback) {
        return Float.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
    }
}
