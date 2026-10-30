package org.questjellyfin.player;

import android.app.Activity;
import android.content.Intent;
import android.media.MediaPlayer;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Surface;
import android.widget.Toast;
import org.json.JSONObject;
import java.util.HashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Displays the Android media player's output on the DeskQuest OpenXR surface. */
public class VrActivity extends Activity {
    static { System.loadLibrary("quest_app"); }
    private native String nativeRun(int width, int height, String format, String order, boolean passthrough, long epoch);
    private native void nativeStop();
    private native long nativeArm();
    private native void nativeVideoSize(long epoch, int width, int height);
    private final ExecutorService xrThread = Executors.newSingleThreadExecutor();
    private final ExecutorService reports = Executors.newSingleThreadExecutor();
    private final Handler ui = new Handler(Looper.getMainLooper());
    private volatile boolean resumed;
    private JSONObject data;
    private String server, device, item;
    private MediaPlayer player;
    private Surface surface;
    private boolean started;
    private boolean completed;
    private boolean awaitingResizedSurface;
    private long lastTicks;
    private int generation;
    private long xrEpoch;

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        try {
            data = new JSONObject(getIntent().getStringExtra("playback"));
            server = getIntent().getStringExtra("server"); device = getIntent().getStringExtra("device"); item = getIntent().getStringExtra("item");
        } catch (Exception e) { finish(); }
    }
    @Override protected void onResume() {
        super.onResume(); if (data == null) return; resumed = true;
        final int current = ++generation;
        final long epoch = nativeArm();
        xrEpoch = epoch;
        xrThread.execute(() -> {
            if (!resumed || current != generation) return;
            String error = nativeRun(data.optInt("width"), data.optInt("height"), data.optString("format"), data.optString("eye_order"), getIntent().getBooleanExtra("passthrough", false), epoch);
            ui.post(() -> { if (resumed && current == generation) { if (error != null && !error.isEmpty()) fail("Immersive playback could not start: " + error); else finish(); } });
        });
    }
    private void fail(String message) {
        setResult(RESULT_CANCELED, new Intent().putExtra("error", message));
        finish();
    }
    /** Called by the native OpenXR thread after the session and surface are ready. */
    public void onXrSurface(Surface nativeSurface) {
        ui.post(() -> {
            if (!resumed || isFinishing()) { nativeSurface.release(); return; }
            surface = nativeSurface;
            try {
                if (player != null) {
                    player.setSurface(surface);
                    android.util.Log.i("Jellyfin3D", "Video surface rebound");
                    if (awaitingResizedSurface) { awaitingResizedSurface = false; beginPlayback(player); }
                    return;
                }
                player = new MediaPlayer(); player.setSurface(surface);
                HashMap<String, String> headers = new HashMap<>(); headers.put("Authorization", data.getString("authorization")); headers.put("android-allow-cross-domain-redirect", "0");
                player.setDataSource(this, Uri.parse(data.getString("url")), headers);
                player.setOnInfoListener((mp, what, extra) -> {
                    if (what == MediaPlayer.MEDIA_INFO_VIDEO_RENDERING_START && resumed && player == mp && !started) {
                        started = true; lastTicks = (long)mp.getCurrentPosition() * 10000;
                        report("start", false); scheduleProgress(generation);
                    }
                    return false;
                });
                player.setOnPreparedListener(mp -> {
                    if (!resumed) return;
                    int width = mp.getVideoWidth(), height = mp.getVideoHeight();
                    if (SurfacePreparation.needsResize(data.optInt("width"), data.optInt("height"), width, height)) {
                        // Rebind while the decoder is prepared but before it starts producing buffers.
                        awaitingResizedSurface = true;
                        nativeVideoSize(xrEpoch, width, height);
                    } else beginPlayback(mp);
                });
                player.setOnCompletionListener(mp -> { lastTicks = (long)mp.getDuration() * 10000; completed = true; finish(); });
                player.setOnErrorListener((mp, what, extra) -> { fail("The headset could not play this video (" + what + ", " + extra + "). Try the 2D preview or another source."); return true; });
                player.prepareAsync();
            } catch (Exception e) { fail("Unable to open the video. Check the server connection and try again."); }
        });
    }
    private void beginPlayback(MediaPlayer mp) {
        long resumeMs = data.optLong("resume") / 10000L;
        Runnable begin = () -> {
            if (!resumed || player != mp) return;
            mp.start();
            int current = generation;
            ui.postDelayed(() -> {
                if (resumed && current == generation && player == mp && !started)
                    fail("Video did not produce a frame. Try the 2D preview or another source.");
            }, 15000);
        };
        if (resumeMs > 0 && resumeMs <= Integer.MAX_VALUE) {
            mp.setOnSeekCompleteListener(ignored -> { mp.setOnSeekCompleteListener(null); begin.run(); });
            mp.seekTo((int)resumeMs);
        } else begin.run();
    }
    /** Detach the producer on the UI thread before OpenXR replaces its swapchain. */
    public boolean onXrDetachSurface() {
        java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.atomic.AtomicBoolean detached = new java.util.concurrent.atomic.AtomicBoolean(false);
        ui.post(() -> {
            try {
                if (resumed && player != null) {
                    player.setSurface(null);
                    if (surface != null) { surface.release(); surface = null; }
                    detached.set(true);
                }
            } catch (IllegalStateException ignored) {} finally { latch.countDown(); }
        });
        try { return latch.await(3, java.util.concurrent.TimeUnit.SECONDS) && detached.get(); }
        catch (InterruptedException e) { Thread.currentThread().interrupt(); return false; }
    }
    /** Controller shortcuts delivered by the OpenXR action set. */
    public void onXrCommand(int command) {
        ui.post(() -> {
            if (!resumed || !started || player == null) return;
            try {
                if (command == 0) {
                    if (player.isPlaying()) player.pause(); else player.start();
                    lastTicks = (long)player.getCurrentPosition() * 10000;
                    report("progress", !player.isPlaying());
                } else if (command == 3) {
                    MediaPlayer.TrackInfo[] tracks = player.getTrackInfo();
                    int selected = player.getSelectedTrack(MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO);
                    java.util.ArrayList<Integer> audio = new java.util.ArrayList<>();
                    for (int i = 0; i < tracks.length; i++) {
                        if (tracks[i].getTrackType() == MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO) audio.add(i);
                    }
                    int[] indices = new int[audio.size()];
                    for (int i = 0; i < indices.length; i++) indices[i] = audio.get(i);
                    int choice = AudioTrackSelector.next(indices, selected);
                    if (choice >= 0) { player.selectTrack(choice); Toast.makeText(this, "Audio: " + tracks[choice].getLanguage(), Toast.LENGTH_SHORT).show(); }
                    else Toast.makeText(this, "No selectable audio track", Toast.LENGTH_SHORT).show();
                } else if (command == 1 || command == 2) {
                    int position = player.getCurrentPosition();
                    long target = Math.max(0L, (long)position + (command == 1 ? -10000L : 10000L));
                    int duration = player.getDuration();
                    if (duration > 0) target = Math.min(target, duration);
                    player.setOnSeekCompleteListener(mp -> {
                        if (player != mp || !started) return;
                        mp.setOnSeekCompleteListener(null);
                        lastTicks = (long)mp.getCurrentPosition() * 10000;
                        report("progress", !mp.isPlaying());
                    });
                    player.seekTo((int)Math.min(target, Integer.MAX_VALUE));
                }
            } catch (IllegalStateException e) { Toast.makeText(this, "Playback control unavailable", Toast.LENGTH_SHORT).show(); }
        });
    }
    // Called on the XR thread before its surface is destroyed.
    public void onXrClosing() {
        java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);
        ui.post(() -> { releasePlayer(); latch.countDown(); });
        try { latch.await(2, java.util.concurrent.TimeUnit.SECONDS); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
    }
    private void releasePlayer() {
        if (player != null) {
            if (started && !completed) { try { lastTicks = (long)player.getCurrentPosition() * 10000; } catch (IllegalStateException ignored) {} }
            player.release(); player = null;
        }
        if (surface != null) { surface.release(); surface = null; }
        if (started) { report("stop", false); started = false; }
    }
    private void report(String action, boolean paused) {
        try {
            JSONObject req = new JSONObject().put("action", action).put("server", server).put("device", device)
                .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json")
                .put("item", item).put("source", data.getString("source")).put("session", data.opt("session"))
                .put("ticks", lastTicks).put("paused", paused).put("method", data.optString("method", "DirectPlay"));
            String raw = req.toString(); reports.execute(() -> MainActivity.nativeRequest(raw));
        } catch (Exception ignored) {}
    }
    private void scheduleProgress(int current) {
        ui.postDelayed(() -> {
            if (current != generation || !resumed || player == null || !started) return;
            lastTicks = (long)player.getCurrentPosition() * 10000; report("progress", !player.isPlaying()); scheduleProgress(current);
        }, 10000);
    }
    @Override protected void onPause() {
        resumed = false; generation++;
        releasePlayer();
        nativeStop();
        super.onPause();
    }
    @Override protected void onDestroy() { xrThread.shutdown(); reports.shutdown(); super.onDestroy(); }
}
