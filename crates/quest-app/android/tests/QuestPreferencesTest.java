package org.questjellyfin.player;

public class QuestPreferencesTest {
    public static void main(String[] args) {
        if (QuestPreferences.screenWidth(Float.NaN) != 4f) throw new AssertionError("invalid saved width");
        if (QuestPreferences.screenDistance(Float.POSITIVE_INFINITY) != 2.2f) throw new AssertionError("invalid saved distance");
        if (QuestPreferences.screenWidth(100f) != 6f) throw new AssertionError("uncomfortable saved width");
        if (QuestPreferences.screenDistance(0f) != 1f) throw new AssertionError("invalid saved near plane");
        if (QuestPreferences.screenWidth(3.5f) != 3.5f) throw new AssertionError("user width changed");
        if (QuestPreferences.columns(-1) != 3 || QuestPreferences.columns(100) != 6) throw new AssertionError("poster grid bounds");
    }
}
