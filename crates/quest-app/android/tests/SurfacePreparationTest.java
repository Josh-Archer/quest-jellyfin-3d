package org.questjellyfin.player;

public class SurfacePreparationTest {
    public static void main(String[] args) {
        if (SurfacePreparation.needsResize(1920, 1080, 1920, 1080)) throw new AssertionError("matching source");
        if (!SurfacePreparation.needsResize(1920, 1080, 1280, 720)) throw new AssertionError("transcoded resolution");
        if (!SurfacePreparation.needsResize(1920, 1080, 1920, 800)) throw new AssertionError("changed height");
        if (SurfacePreparation.needsResize(1920, 1080, 0, 0)) throw new AssertionError("unknown dimensions");
    }
}
