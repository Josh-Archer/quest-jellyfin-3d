package org.questjellyfin.player;

import android.app.Activity;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.content.SharedPreferences;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.util.Base64;
import android.net.Uri;
import java.util.HashMap;
import android.view.View;
import android.widget.*;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Native Jellyfin API client and 2D playback preview; immersive output is under development. */
public class MainActivity extends Activity {
    static { System.loadLibrary("quest_app"); }
    static native String nativeRequest(String json);
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final ExecutorService images = Executors.newFixedThreadPool(3);
    private final Handler ui = new Handler(Looper.getMainLooper());
    private SharedPreferences prefs;
    private LinearLayout root;
    private TextView status;
    private String server, device, pairingSecret, activeItem, activeSource, activeSession, activeMethod;
    private VideoView video;
    private boolean started;
    private int generation;
    private int lastPosition;
    private String searchTerm = "";

    interface Done { void apply(JSONObject data) throws Exception; }
    private void request(String action, JSONObject extra, Done done) {
        final String targetServer = server;
        final int requestGeneration = generation;
        try {
            JSONObject req = extra == null ? new JSONObject() : extra;
            req.put("action", action).put("server", targetServer).put("device", device)
                .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json");
            String raw = req.toString();
            io.execute(() -> {
                try {
                    JSONObject reply = new JSONObject(nativeRequest(raw));
                    ui.post(() -> {
                        if (isFinishing() || !targetServer.equals(server) || requestGeneration != generation) return;
                        try {
                            if (!reply.optBoolean("ok")) { showStatus(reply.optString("error", "Request failed")); return; }
                            done.apply(reply.getJSONObject("data"));
                        } catch (Exception ignored) { showStatus("Could not process response"); }
                    });
                } catch (Exception ignored) { ui.post(() -> showStatus("Could not process response")); }
            });
        } catch (Exception ignored) { showStatus("Could not prepare request"); }
    }
    private void showStatus(String text) { if (status != null) status.setText(text); }
    private JSONObject value(String key, Object val) { JSONObject o = new JSONObject(); try { o.put(key, val); } catch (Exception ignored) {} return o; }
    private TextView label(String text) { TextView t = new TextView(this); t.setText(text); t.setTextSize(18); t.setTextColor(Color.WHITE); return t; }
    private Button button(String text, View.OnClickListener click) { Button b = new Button(this); b.setText(text); b.setOnClickListener(click); return b; }
    private void page(String title) {
        root = new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL); root.setPadding(30, 20, 30, 20); root.setBackgroundColor(Color.rgb(23, 22, 32));
        root.addView(label(title)); status = label(""); root.addView(status); setContentView(root);
    }
    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved); prefs = getSharedPreferences("app", MODE_PRIVATE);
        device = prefs.getString("device", null);
        if (device == null) { device = UUID.randomUUID().toString(); prefs.edit().putString("device", device).apply(); }
        server = prefs.getString("server", ""); home();
    }
    private void home() {
        generation++; pairingSecret = null; page("Jellyfin 3D • Setup");
        EditText address = new EditText(this); address.setSingleLine(); address.setTextColor(Color.WHITE); address.setHintTextColor(Color.LTGRAY);
        address.setHint("Jellyfin server URL (https://...)"); address.setText(server); root.addView(address);
        root.addView(button("Connect", v -> {
            server = address.getText().toString().trim();
            if (server.isEmpty()) { showStatus("Enter your Jellyfin server URL"); return; }
            prefs.edit().putString("server", server).apply();
            showStatus("Connecting…"); request("status", null, d -> { if (d.getBoolean("signed_in")) browse(0); else pair(); });
        }));
        root.addView(label("Uses Jellyfin Quick Connect. Immersive playback currently supports flat mono, SBS and over-under video."));
    }
    private void pair() {
        int current = ++generation; page("Connect to Jellyfin"); showStatus("Requesting a code…");
        root.addView(button("Back", v -> home()));
        request("initiate", null, data -> {
            if (generation != current) return;
            pairingSecret = data.getString("secret");
            showStatus("In Jellyfin, open Settings → Quick Connect and enter code: " + data.getString("code"));
            poll(current);
        });
    }
    private void poll(int current) {
        ui.postDelayed(() -> {
            if (generation != current || pairingSecret == null) return;
            request("poll", value("secret", pairingSecret), data -> {
                if (generation != current) return;
                if (data.getBoolean("authorized")) { pairingSecret = null; browse(0); }
                else poll(current);
            });
        }, 3000);
    }
    private void loadPoster(String id, ImageView image, int current) {
        try {
            String raw = value("action", "image").put("item", id).put("server", server).put("device", device)
                .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json").toString();
            images.execute(() -> {
                try {
                    JSONObject reply = new JSONObject(nativeRequest(raw));
                    if (!reply.optBoolean("ok")) return;
                    JSONObject data = reply.getJSONObject("data");
                    if (data.isNull("image")) return;
                    byte[] bytes = Base64.decode(data.getString("image"), Base64.DEFAULT);
                    Bitmap poster = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
                    if (poster != null) ui.post(() -> { if (!isFinishing() && generation == current) image.setImageBitmap(poster); });
                } catch (Exception ignored) { /* A missing poster leaves the title usable. */ }
            });
        } catch (Exception ignored) {}
    }
    private void browse(int start) {
        int current = ++generation; page("Jellyfin library");
        LinearLayout row = new LinearLayout(this);
        row.addView(button("Server", v -> home()));
        row.addView(button("Sign out", v -> request("signout", null, d -> home())));
        EditText search = new EditText(this); search.setSingleLine(); search.setTextColor(Color.WHITE); search.setHintTextColor(Color.LTGRAY); search.setHint("Search your library"); search.setText(searchTerm);
        row.addView(search, new LinearLayout.LayoutParams(0, -2, 1));
        row.addView(button("Search", v -> { searchTerm = search.getText().toString().trim(); browse(0); }));
        root.addView(row);
        ScrollView scroller = new ScrollView(this); LinearLayout list = new LinearLayout(this); list.setOrientation(LinearLayout.VERTICAL); scroller.addView(list); root.addView(scroller, new LinearLayout.LayoutParams(-1, 0, 1));
        showStatus("Loading library…");
        JSONObject query = value("start", start);
        try { query.put("search", searchTerm); } catch (Exception ignored) {}
        request("browse", query, data -> {
            JSONArray items = data.getJSONArray("items");
            showStatus(items.length() == 0 ? "No titles found." : "Select a title to watch");
            LinearLayout cards = null;
            for (int i = 0; i < items.length(); i++) {
                if (i % 4 == 0) { cards = new LinearLayout(this); list.addView(cards); }
                JSONObject item = items.getJSONObject(i); String id = item.getString("id");
                String name = item.optString("name", "Untitled");
                LinearLayout card = new LinearLayout(this); card.setOrientation(LinearLayout.VERTICAL); card.setPadding(8, 8, 8, 8);
                card.setContentDescription("Watch " + name); card.setOnClickListener(v -> prepare(id));
                ImageView poster = new ImageView(this); poster.setBackgroundColor(Color.rgb(50, 48, 67)); poster.setScaleType(ImageView.ScaleType.CENTER_CROP);
                int height = (int)(230 * getResources().getDisplayMetrics().density);
                card.addView(poster, new LinearLayout.LayoutParams(-1, height));
                TextView caption = label(name); caption.setMaxLines(2); card.addView(caption);
                cards.addView(card, new LinearLayout.LayoutParams(0, -2, 1));
                loadPoster(id, poster, current);
            }
            if (cards != null && items.length() % 4 != 0) {
                for (int i = items.length() % 4; i < 4; i++) cards.addView(new View(this), new LinearLayout.LayoutParams(0, 1, 1));
            }
            LinearLayout pages = new LinearLayout(this);
            if (start > 0) pages.addView(button("Previous", v -> browse(Math.max(0, start - 50))));
            if (!data.isNull("next")) { int next = data.getInt("next"); pages.addView(button("Next", v -> browse(next))); }
            list.addView(pages);
        });
    }
    private void prepare(String item) {
        int current = ++generation; page("Preparing playback"); showStatus("Contacting Jellyfin…"); root.addView(button("Back", v -> browse(0)));
        request("prepare", value("item", item), data -> { if (generation == current) choosePlayback(item, data); });
    }
    private void choosePlayback(String item, JSONObject data) {
        page("Choose playback");
        String projection = data.optString("projection", "Flat");
        String format = data.optString("format", "Mono");
        showStatus("Format: " + format + " • Projection: " + projection + " • " + data.optString("method"));
        boolean supported = projection.equals("Flat") && !format.equals("Mvc") && !format.equals("MvHevc") && !format.equals("Unspecified3d") && data.optInt("width", 0) > 0 && data.optInt("height", 0) > 0;
        CheckBox passthrough = new CheckBox(this); passthrough.setText("Passthrough background"); passthrough.setTextColor(Color.WHITE); root.addView(passthrough);
        root.addView(label("In immersive mode: trigger pauses or resumes; thumbstick left/right seeks 10 seconds; grip cycles audio tracks."));
        if (!data.optString("method").equals("DirectPlay")) root.addView(label("Server transcoding may change resolution, bitrate, or HDR. Immersive playback is experimental."));
        if (format.equals("Mvc") || format.equals("MvHevc")) root.addView(label("This multiview format is not supported in stereo. The 2D preview may show only one view."));
        if (supported) root.addView(button("Watch in immersive 3D", v -> {
            Intent intent = new Intent(this, VrActivity.class);
            intent.addCategory("com.oculus.intent.category.VR");
            intent.addCategory("org.khronos.openxr.intent.category.IMMERSIVE_HMD");
            intent.putExtra("server", server).putExtra("device", device).putExtra("item", item).putExtra("playback", data.toString()).putExtra("passthrough", passthrough.isChecked());
            showStatus("Starting immersive playback…");
            startActivityForResult(intent, 1);
        }));
        else root.addView(label("Immersive playback is unavailable for this format or missing video dimensions."));
        root.addView(button("2D preview", v -> { try { playback(item, data); } catch (Exception e) { showStatus("Could not start playback"); } }));
        root.addView(button("Back", v -> browse(0)));
    }
    @Override protected void onActivityResult(int requestCode, int resultCode, Intent result) {
        super.onActivityResult(requestCode, resultCode, result);
        if (requestCode == 1) {
            String error = result == null ? null : result.getStringExtra("error");
            showStatus(error == null ? "Playback closed. Select a playback option." : error);
        }
    }
    private JSONObject playbackData(long ticks) {
        JSONObject o = value("item", activeItem);
        try { o.put("source", activeSource).put("session", activeSession == null ? JSONObject.NULL : activeSession).put("ticks", ticks).put("method", activeMethod); } catch (Exception ignored) {}
        return o;
    }
    private void playback(String item, JSONObject data) throws Exception {
        activeItem = item; activeMethod = data.optString("method", "DirectPlay"); activeSource = data.getString("source"); activeSession = data.isNull("session") ? null : data.getString("session");
        long resumeMs = data.optLong("resume", 0) / 10000L; int current = ++generation;
        page("2D preview • " + data.optString("format", "Mono"));
        root.addView(button("Back to library", v -> { stopPlayback(); browse(0); }));
        video = new VideoView(this); root.addView(video, new LinearLayout.LayoutParams(-1, 0, 1));
        MediaController controls = new MediaController(this); controls.setAnchorView(video); video.setMediaController(controls);
        video.setOnPreparedListener(mp -> {
            if (current != generation) return;
            if (resumeMs > 0 && resumeMs <= Integer.MAX_VALUE) video.seekTo((int) resumeMs);
            video.start(); lastPosition = video.getCurrentPosition(); started = true;
            request("start", playbackData(lastPosition * 10000L), ignored -> {}); progress(current);
        });
        video.setOnCompletionListener(mp -> { stopPlayback(); browse(0); });
        video.setOnErrorListener((mp, what, extra) -> { stopPlayback(); showStatus("This device could not play the selected source."); return true; });
        showStatus("Opening video…"); HashMap<String, String> headers = new HashMap<>();
        headers.put("Authorization", data.getString("authorization"));
        headers.put("android-allow-cross-domain-redirect", "0");
        video.setVideoURI(Uri.parse(data.getString("url")), headers);
    }
    private void progress(int current) {
        ui.postDelayed(() -> {
            if (current != generation || video == null || !started) return;
            lastPosition = video.getCurrentPosition(); JSONObject report = playbackData(lastPosition * 10000L);
            try { report.put("paused", !video.isPlaying()); } catch (Exception ignored) {}
            request("progress", report, ignored -> {}); progress(current);
        }, 10000);
    }
    private void stopPlayback() {
        generation++;
        if (video != null) { if (started) lastPosition = video.getCurrentPosition(); video.stopPlayback(); video = null; }
        if (started) { request("stop", playbackData(lastPosition * 10000L), ignored -> {}); started = false; }
    }
    @Override protected void onStop() { super.onStop(); if (video != null) { stopPlayback(); home(); } }
    @Override protected void onDestroy() { generation++; io.shutdown(); images.shutdownNow(); super.onDestroy(); }
}
