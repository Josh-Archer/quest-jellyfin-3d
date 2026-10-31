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
    private volatile long xrEpoch;
    private PlaybackGate playbackGate = new PlaybackGate();
    private boolean prepared;
    private boolean readyToPlay;
    private volatile String transportStatus = "Preparing video…";
    private volatile String playLabel = "Pause";
    private volatile float progressFraction;
    private volatile String screenStatus = "Flat screen";
    private volatile String audioLabel = "Audio";
    private volatile boolean controlsSubmitted;
    private volatile boolean buffering;
    private final android.content.BroadcastReceiver screenOff = new android.content.BroadcastReceiver() {
        @Override public void onReceive(android.content.Context context, Intent intent) {
            if (Intent.ACTION_SCREEN_OFF.equals(intent.getAction())) interruptPlayback();
        }
    };

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        android.widget.FrameLayout window = new android.widget.FrameLayout(this);
        window.setBackgroundColor(android.graphics.Color.BLACK); setContentView(window);
        android.util.Log.i("Jellyfin3D", "VR created: display=" + getDisplay().getDisplayId() + " task=" + getTaskId());
        registerReceiver(screenOff, new android.content.IntentFilter(Intent.ACTION_SCREEN_OFF), android.content.Context.RECEIVER_NOT_EXPORTED);
        loadPlaybackIntent();
    }
    private void loadPlaybackIntent() {
        failure = null; outcomeReported = false; completed = false; videoLayerSubmitted = false;
        playbackGate = new PlaybackGate(); lastTicks = 0;
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
        xrEpoch = epoch; videoLayerSubmitted = false; controlsSubmitted = false;
        prepared = false; readyToPlay = false;
        buffering = false; transportStatus = "Preparing video…";
        screenStatus = String.format(java.util.Locale.ROOT, "%.1f m · %s", getIntent().getFloatExtra("screen_width", 4f), initialCurve() > 0 ? "curved" : "flat");
        updateTransport(current);
        ui.postDelayed(() -> {
            if (resumed && generation == current && playbackGate.canPlay() && !videoLayerSubmitted) fail("Cinema could not present video on the headset. Try reopening Cinema or use the Quest panel.");
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
        if (!focused) interruptPlayback();
    }
    private void interruptPlayback() { playbackGate.interrupt(); applyPlaybackGate(); }
    /** Transport atlas; pixels 0..191 are the panel, the native code adds shared DeskQuest handles. */
    public byte[] controlsPixels() {
        android.graphics.Bitmap image = android.graphics.Bitmap.createBitmap(512, 256, android.graphics.Bitmap.Config.ARGB_8888);
        android.graphics.Canvas canvas = new android.graphics.Canvas(image);
        android.graphics.Paint p = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
        p.setColor(android.graphics.Color.rgb(19, 23, 34)); canvas.drawRoundRect(0, 0, 512, 192, 14, 14, p);
        p.setColor(android.graphics.Color.WHITE); p.setTextSize(17); p.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
        canvas.drawText(transportStatus, 12, 25, p);
        if (transportStatus.startsWith("Preparing") || buffering) {
            p.setColor(android.graphics.Color.rgb(0, 164, 220)); p.setStyle(android.graphics.Paint.Style.STROKE); p.setStrokeWidth(3);
            canvas.drawArc(422, 10, 440, 28, (android.os.SystemClock.uptimeMillis() / 5) % 360, 260, false, p); p.setStyle(android.graphics.Paint.Style.FILL);
        }
        p.setTextSize(14); canvas.drawText("Hide", 461, 25, p);
        p.setColor(android.graphics.Color.rgb(55, 63, 78)); canvas.drawRoundRect(12, 39, 500, 47, 4, 4, p);
        p.setColor(android.graphics.Color.rgb(0, 164, 220)); canvas.drawRoundRect(12, 39, 12 + 488 * progressFraction, 47, 4, 4, p);
        String[][] labels = {{playLabel, "−10 sec", "+10 sec", audioLabel, "Exit"}, {"Smaller", "Larger", "Flatter", "Curve +", "Recenter"}};
        for (int row = 0; row < 2; row++) for (int column = 0; column < 5; column++) {
            float left = column * 102.4f + 4, top = row == 0 ? 60 : 106;
            p.setColor(android.graphics.Color.rgb(39, 51, 68)); canvas.drawRoundRect(left, top, left + 94.4f, top + 40, 7, 7, p);
            p.setColor(android.graphics.Color.WHITE); p.setTextSize(16); p.setTextAlign(android.graphics.Paint.Align.CENTER);
            canvas.drawText(labels[row][column], left + 47.2f, top + 26, p);
        }
        p.setTextAlign(android.graphics.Paint.Align.CENTER); p.setTextSize(13); p.setColor(android.graphics.Color.LTGRAY);
        canvas.drawText("Grip / pinch edges: move · Corners: resize", 256, 165, p);
        canvas.drawText(screenStatus + " · Click / pinch movie to show controls", 256, 183, p);
        // Reserved loft materials, sampled only by the environment compositor surfaces.
        p.setColor(android.graphics.Color.rgb(53, 39, 28)); canvas.drawRect(128, 204, 256, 256, p);
        for (int y = 204; y < 256; y += 4) {
            p.setColor(android.graphics.Color.rgb(78 + y % 17, 57 + y % 9, 38)); canvas.drawRect(129, y, 255, y + 3, p);
            p.setColor(android.graphics.Color.rgb(35, 28, 24)); canvas.drawLine(145 + y % 69, y, 145 + y % 69, y + 3, p);
        }
        p.setColor(android.graphics.Color.rgb(75, 68, 63)); canvas.drawRect(256, 192, 384, 256, p);
        p.setStrokeWidth(1); p.setColor(android.graphics.Color.rgb(57, 52, 48));
        for (int y = 192; y <= 256; y += 4) { canvas.drawLine(256, y, 384, y, p); for (int x = 256 + (y % 8 == 0 ? 0 : 8); x < 384; x += 16) canvas.drawLine(x, y, x, y+4, p); }
        p.setColor(android.graphics.Color.rgb(19, 29, 44)); canvas.drawRect(279, 201, 361, 243, p);
        p.setShader(new android.graphics.LinearGradient(0, 202, 0, 241, android.graphics.Color.rgb(51, 66, 93), android.graphics.Color.rgb(14, 19, 30), android.graphics.Shader.TileMode.CLAMP));
        canvas.drawRect(281, 203, 359, 241, p); p.setShader(null);
        p.setColor(android.graphics.Color.rgb(20, 22, 28)); for (int x = 282; x < 359; x += 6) canvas.drawRect(x, 229 - x % 17, x + 5, 241, p);
        p.setColor(android.graphics.Color.rgb(153, 131, 85)); for (int x=285; x<358; x+=7) canvas.drawRect(x,234,x+1,236,p);
        p.setColor(android.graphics.Color.rgb(27, 29, 32)); p.setStrokeWidth(2); canvas.drawLine(320, 202, 320, 243, p); canvas.drawLine(280, 221, 360, 221, p);
        p.setColor(android.graphics.Color.rgb(34, 37, 42)); canvas.drawRect(384,192,512,256,p);
        p.setColor(android.graphics.Color.rgb(47, 51, 57)); for (int x=387;x<512;x+=32) canvas.drawRoundRect(x,195,x+26,253,3,3,p);
        int[] pixels = new int[512 * 256]; image.getPixels(pixels, 0, 512, 0, 0, 512, 256); image.recycle();
        byte[] rgba = new byte[pixels.length * 4];
        for (int i = 0; i < pixels.length; i++) { int value = pixels[i]; rgba[4*i] = (byte)(value >> 16); rgba[4*i+1] = (byte)(value >> 8); rgba[4*i+2] = (byte)value; rgba[4*i+3] = (byte)(value >> 24); }
        return rgba;
    }
    public void onXrControlsSubmitted(long epoch) { if (epoch == xrEpoch) controlsSubmitted = true; }
    public boolean hasSubmittedControls() { return controlsSubmitted; }
    public void onXrScreenChanged(float width, float distance, float angle) {
        screenStatus = String.format(java.util.Locale.ROOT, "%.1f m · %s", width, angle > 0 ? "curved" : "flat");
        ui.post(() -> getSharedPreferences("app", MODE_PRIVATE).edit()
            .putFloat("screen_width", QuestPreferences.screenWidth(width))
            .putFloat("screen_distance", QuestPreferences.screenDistance(distance)).putFloat("screen_curve", angle).apply());
    }
    public float initialCurve() { return getSharedPreferences("app", MODE_PRIVATE).getFloat("screen_curve", 0f); }
    public boolean isLoft() { return "loft".equals(getIntent().getStringExtra("environment")); }
    /** OpenXR focus is lost when removed or when the system menu takes over, without onPause. */
    public void onXrFocusChanged(long epoch, boolean focused) {
        ui.post(() -> {
            if (!resumed || epoch != xrEpoch) return;
            playbackGate.focus(focused);
            applyPlaybackGate();
            android.util.Log.i("Jellyfin3D", "XR focus=" + focused + " playbackAllowed=" + playbackGate.canPlay());
        });
    }
    private void applyPlaybackGate() {
        if (player == null || !readyToPlay) return;
        try {
            if (playbackGate.canPlay()) { if (!player.isPlaying()) player.start(); }
            else if (player.isPlaying()) player.pause();
            if (started) { lastTicks = (long)player.getCurrentPosition() * 10000; report("progress", !player.isPlaying()); }
        } catch (IllegalStateException ignored) {}
    }
    public boolean isVideoPlaying() { return player != null && readyToPlay && player.isPlaying(); }
    public void onXrSeek(int fraction) {
        ui.post(() -> {
            if (!resumed || !readyToPlay || player == null) return;
            try {
                int duration = player.getDuration();
                if (duration > 0) player.seekTo((long)duration * Math.max(0, Math.min(1000, fraction)) / 1000, MediaPlayer.SEEK_CLOSEST);
            } catch (IllegalStateException ignored) {}
        });
    }
    private void updateTransport(int current) {
        if (!resumed || generation != current) return;
        if (player != null && prepared) try {
            int position = player.getCurrentPosition(), duration = player.getDuration();
            boolean playing = readyToPlay && player.isPlaying();
            playLabel = playing ? "Pause" : "Play";
            progressFraction = duration > 0 ? Math.max(0f, Math.min(1f, (float)position / duration)) : 0f;
            transportStatus = (buffering ? "Buffering  " : playing ? "Playing  " : "Paused  ") + clock(position) + " / " + clock(duration);
        } catch (IllegalStateException ignored) {}
        ui.postDelayed(() -> updateTransport(current), 250);
    }
    private static String clock(int millis) { int seconds = Math.max(0, millis / 1000); return String.format(java.util.Locale.ROOT, "%d:%02d", seconds / 60, seconds % 60); }
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
                    if (what == MediaPlayer.MEDIA_INFO_BUFFERING_START) { buffering = true; nativeVideoState(xrEpoch, 2); }
                    else if (what == MediaPlayer.MEDIA_INFO_BUFFERING_END && started) { buffering = false; nativeVideoState(xrEpoch, 1); }
                    return false;
                });
                player.setOnPreparedListener(mp -> {
                    if (!resumed || player != mp) return;
                    prepared = true;
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
            report("start", !player.isPlaying()); scheduleProgress(generation);
        });
    }
    public boolean hasSubmittedVideoLayer() { return videoLayerSubmitted; }
    private void beginPlayback(MediaPlayer mp) {
        long resumeMs = Math.max(lastTicks, data.optLong("resume")) / 10000L;
        Runnable begin = () -> {
            if (!resumed || isFinishing() || player != mp) return;
            readyToPlay = true; applyPlaybackGate();
            int current = generation;
            ui.postDelayed(() -> {
                if (resumed && current == generation && player == mp && playbackGate.canPlay() && !videoLayerSubmitted)
                    fail("Cinema did not present a video frame. Try reopening Cinema or use the Quest panel.");
            }, 15000);
        };
        if (resumeMs > 0 && resumeMs <= Integer.MAX_VALUE) {
            mp.setOnSeekCompleteListener(ignored -> { mp.setOnSeekCompleteListener(null); begin.run(); });
            mp.seekTo(resumeMs, MediaPlayer.SEEK_CLOSEST);
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
            if (!resumed) return;
            if (command == 4) { finish(); return; }
            if (!readyToPlay || player == null) return;
            try {
                if (command == 0) {
                    playbackGate.toggle(); applyPlaybackGate();
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
                    if (choice >= 0) { player.selectTrack(choice); audioLabel = "Audio " + tracks[choice].getLanguage(); }
                    else audioLabel = "No audio";
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
                    player.seekTo(Math.min(target, Integer.MAX_VALUE), MediaPlayer.SEEK_CLOSEST);
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
        awaitingResizedSurface = false; prepared = false; readyToPlay = false;
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
        playbackGate.focus(false);
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
        unregisterReceiver(screenOff);
        if (!isChangingConfigurations()) deliverOutcome();
        xrThread.shutdown(); reports.shutdown(); super.onDestroy();
    }
}
