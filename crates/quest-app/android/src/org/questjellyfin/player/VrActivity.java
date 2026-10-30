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
    private native String nativeRun(int width, int height, String format, String order, boolean passthrough, float screenWidth, float screenDistance, long epoch);
    private native void nativeStop();
    private native void nativeVideoState(long epoch, int state);
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
    private String failure;
    private boolean outcomeReported;
    private volatile boolean videoLayerSubmitted;
    private boolean awaitingResizedSurface;
    private long lastTicks;
    private int generation;
    private long xrEpoch;

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        android.widget.FrameLayout window = new android.widget.FrameLayout(this);
        window.setBackgroundColor(android.graphics.Color.BLACK); setContentView(window);
        android.util.Log.i("Jellyfin3D", "VR created: display=" + getDisplay().getDisplayId() + " task=" + getTaskId());
        loadPlaybackIntent();
    }
    private void loadPlaybackIntent() {
        failure = null; outcomeReported = false; completed = false; videoLayerSubmitted = false;
        try {
            data = new JSONObject(getIntent().getStringExtra("playback"));
            server = getIntent().getStringExtra("server"); device = getIntent().getStringExtra("device"); item = getIntent().getStringExtra("item");
        } catch (Exception e) { fail("Unable to read the selected playback source."); }
    }
    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent); deliverOutcome(); setIntent(intent); loadPlaybackIntent();
    }
    @Override protected void onResume() {
        super.onResume(); android.util.Log.i("Jellyfin3D", "VR resumed"); if (data == null) return; resumed = true;
        final int current = ++generation;
        final long epoch = nativeArm();
        xrEpoch = epoch; videoLayerSubmitted = false;
        ui.postDelayed(() -> {
            if (resumed && generation == current && !videoLayerSubmitted) fail("Cinema could not present video on the headset. Try reopening Cinema or use the Quest panel.");
        }, 30000);
        xrThread.execute(() -> {
            if (!resumed || current != generation) return;
            String error = nativeRun(data.optInt("width"), data.optInt("height"), data.optString("format"), data.optString("eye_order"), getIntent().getBooleanExtra("passthrough", false), QuestPreferences.screenWidth(getIntent().getFloatExtra("screen_width", 4f)), QuestPreferences.screenDistance(getIntent().getFloatExtra("screen_distance", 2.2f)), epoch);
            ui.post(() -> { if (resumed && current == generation) { if (error != null && !error.isEmpty()) fail("Immersive playback could not start: " + error); else finish(); } });
        });
    }
    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        android.util.Log.i("Jellyfin3D", "VR window focus=" + focused);
    }
    /** RGBA loading animation reused by DeskQuest's textured compositor overlays. */
    public byte[] loadingPixels(int phase) {
        android.graphics.Bitmap image = android.graphics.Bitmap.createBitmap(512, 256, android.graphics.Bitmap.Config.ARGB_8888);
        android.graphics.Canvas canvas = new android.graphics.Canvas(image);
        canvas.drawColor(android.graphics.Color.rgb(20, 22, 33));
        android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
        paint.setColor(android.graphics.Color.rgb(0, 164, 220)); paint.setTextAlign(android.graphics.Paint.Align.CENTER);
        paint.setTextSize(32); paint.setTypeface(android.graphics.Typeface.DEFAULT_BOLD); canvas.drawText("Jellyfin", 256, 54, paint);
        paint.setStyle(android.graphics.Paint.Style.STROKE); paint.setStrokeWidth(7);
        canvas.drawArc(230, 78, 282, 130, phase * 45, 260, false, paint);
        paint.setStyle(android.graphics.Paint.Style.FILL); paint.setColor(android.graphics.Color.WHITE); paint.setTextSize(24);
        canvas.drawText("Loading video…", 256, 176, paint); paint.setTextSize(16);
        canvas.drawText("Use system Back to cancel", 256, 215, paint);
        int[] pixels = new int[512 * 256]; image.getPixels(pixels, 0, 512, 0, 0, 512, 256); image.recycle();
        byte[] rgba = new byte[pixels.length * 4];
        for (int i = 0; i < pixels.length; i++) { int p = pixels[i]; rgba[4*i] = (byte)(p >> 16); rgba[4*i+1] = (byte)(p >> 8); rgba[4*i+2] = (byte)p; rgba[4*i+3] = (byte)(p >> 24); }
        return rgba;
    }
    private void fail(String message) {
        android.util.Log.e("Jellyfin3D", message);
        failure = message;
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
                    if (!resumed || player != mp) return false;
                    if (what == MediaPlayer.MEDIA_INFO_VIDEO_RENDERING_START && resumed && player == mp && !started) {
                        nativeVideoState(xrEpoch, 1);
                        android.util.Log.i("Jellyfin3D", "Decoder produced first video frame");
                        lastTicks = (long)mp.getCurrentPosition() * 10000;
                    }
                    if (what == MediaPlayer.MEDIA_INFO_BUFFERING_START) nativeVideoState(xrEpoch, 2);
                    else if (what == MediaPlayer.MEDIA_INFO_BUFFERING_END && started) nativeVideoState(xrEpoch, 1);
                    return false;
                });
                player.setOnPreparedListener(mp -> {
                    if (!resumed || player != mp) return;
                    int width = mp.getVideoWidth(), height = mp.getVideoHeight();
                    if (SurfacePreparation.needsResize(data.optInt("width"), data.optInt("height"), width, height)) {
                        // Rebind while the decoder is prepared but before it starts producing buffers.
                        awaitingResizedSurface = true;
                        nativeVideoSize(xrEpoch, width, height);
                    } else beginPlayback(mp);
                });
                player.setOnCompletionListener(mp -> { if (player == mp) { lastTicks = (long)mp.getDuration() * 10000; completed = true; finish(); } });
                player.setOnErrorListener((mp, what, extra) -> { fail("The headset could not play this video (" + what + ", " + extra + "). Try the 2D preview or another source."); return true; });
                player.prepareAsync();
            } catch (Exception e) { fail("Unable to open the video. Check the server connection and try again."); }
        });
    }
    /** A decoder callback alone is not proof that a compositor layer was submitted. */
    public void onXrVideoLayerSubmitted(long epoch) {
        ui.post(() -> {
            if (!resumed || epoch != xrEpoch || player == null || videoLayerSubmitted) return;
            videoLayerSubmitted = true; started = true;
            lastTicks = (long)player.getCurrentPosition() * 10000;
            report("start", false); scheduleProgress(generation);
        });
    }
    public boolean hasSubmittedVideoLayer() { return videoLayerSubmitted; }
    private void beginPlayback(MediaPlayer mp) {
        long resumeMs = data.optLong("resume") / 10000L;
        Runnable begin = () -> {
            if (!resumed || isFinishing() || player != mp) return;
            mp.start();
            int current = generation;
            ui.postDelayed(() -> {
                if (resumed && current == generation && player == mp && !videoLayerSubmitted)
                    fail("Cinema did not present a video frame. Try reopening Cinema or use the Quest panel.");
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
        awaitingResizedSurface = false;
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
        android.util.Log.i("Jellyfin3D", "VR paused");
        resumed = false; generation++;
        releasePlayer();
        nativeStop();
        super.onPause();
    }
    private void deliverOutcome() {
        if (outcomeReported) return;
        outcomeReported = true;
        android.os.ResultReceiver receiver = getIntent().getParcelableExtra("cinema_result");
        if (receiver != null) {
            Bundle result = new Bundle(); if (failure != null) result.putString("error", failure);
            receiver.send(failure == null ? RESULT_OK : RESULT_CANCELED, result);
        }
    }
    @Override protected void onDestroy() {
        if (!isChangingConfigurations()) deliverOutcome();
        xrThread.shutdown(); reports.shutdown(); super.onDestroy();
    }
}
