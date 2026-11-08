package org.questjellyfin.player;

public class QuestPreferencesTest {
    public static void main(String[] args) {
        if (QuestPreferences.stereoAlignment(Float.NaN)!=0f || QuestPreferences.stereoAlignment(Float.POSITIVE_INFINITY)!=0f) throw new AssertionError("Invalid stereo calibration");
        if (QuestPreferences.stereoAlignment(99f)!=8f || QuestPreferences.stereoAlignment(-99f)!=-8f) throw new AssertionError("Unbounded stereo calibration");
        if (QuestPreferences.stereoAlignment(-.25f)!=-.5f || QuestPreferences.stereoAlignment(.25f)!=.5f) throw new AssertionError("Inconsistent signed calibration steps");
        if (QuestPreferences.screenWidth(Float.NaN) != 4f) throw new AssertionError("invalid saved width");
        if (QuestPreferences.screenDistance(Float.POSITIVE_INFINITY) != 2.2f) throw new AssertionError("invalid saved distance");
        if (QuestPreferences.screenWidth(100f) != 6f) throw new AssertionError("uncomfortable saved width");
        if (QuestPreferences.screenDistance(0f) != 1f) throw new AssertionError("invalid saved near plane");
        if (QuestPreferences.screenWidth(3.5f) != 3.5f) throw new AssertionError("user width changed");
        if (QuestPreferences.columns(-1) != 3 || QuestPreferences.columns(100) != 6) throw new AssertionError("poster grid bounds");
    }
}
