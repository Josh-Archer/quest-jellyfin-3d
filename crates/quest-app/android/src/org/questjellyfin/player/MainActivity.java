package org.questjellyfin.player;

import android.app.Activity;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.content.SharedPreferences;
import android.content.Intent;
import android.graphics.Color;
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
        try {
            JSONObject req = extra == null ? new JSONObject() : extra;
            req.put("action", action).put("server", targetServer).put("device", device)
                .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json");
            String raw = req.toString();
            io.execute(() -> {
                try {
                    JSONObject reply = new JSONObject(nativeRequest(raw));
                    ui.post(() -> {
                        if (isFinishing() || !targetServer.equals(server)) return;
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
    private void browse(int start) {
        generation++; page("Jellyfin library");
        LinearLayout row = new LinearLayout(this);
        row.addView(button("Server", v -> home()));
        row.addView(button("Sign out", v -> request("signout", null, d -> home())));
        root.addView(row);
        EditText search = new EditText(this); search.setSingleLine(); search.setTextColor(Color.WHITE); search.setHintTextColor(Color.LTGRAY); search.setHint("Search your library"); search.setText(searchTerm); root.addView(search);
        root.addView(button("Search", v -> { searchTerm = search.getText().toString().trim(); browse(0); }));
        ScrollView scroller = new ScrollView(this); LinearLayout list = new LinearLayout(this); list.setOrientation(LinearLayout.VERTICAL); scroller.addView(list); root.addView(scroller, new LinearLayout.LayoutParams(-1, 0, 1));
        showStatus("Loading…");
        JSONObject query = value("start", start);
        try { query.put("search", searchTerm); } catch (Exception ignored) {}
        request("browse", query, data -> {
            showStatus("Select a title for a 2D playback preview");
            JSONArray items = data.getJSONArray("items");
            for (int i = 0; i < items.length(); i++) {
                JSONObject item = items.getJSONObject(i); String id = item.getString("id");
                String format = item.optString("format", ""); if ("null".equals(format)) format = "";
                list.addView(button(item.optString("name", "Untitled") + (format.isEmpty() ? "" : " • " + format), v -> prepare(id)));
            }
            if (start > 0) list.addView(button("Previous", v -> browse(Math.max(0, start - 50))));
            if (!data.isNull("next")) { int next = data.getInt("next"); list.addView(button("Next", v -> browse(next))); }
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
        boolean supported = data.optString("method").equals("DirectPlay") && projection.equals("Flat") && !format.equals("Mvc") && !format.equals("MvHevc") && !format.equals("Unspecified3d") && data.optInt("width", 0) > 0 && data.optInt("height", 0) > 0;
        CheckBox passthrough = new CheckBox(this); passthrough.setText("Passthrough background"); passthrough.setTextColor(Color.WHITE); root.addView(passthrough);
        root.addView(label("In immersive mode: trigger pauses or resumes; thumbstick left/right seeks 10 seconds; grip cycles audio tracks."));
        if (supported) root.addView(button("Watch in immersive 3D", v -> {
            Intent intent = new Intent(this, VrActivity.class);
            intent.addCategory("com.oculus.intent.category.VR");
            intent.addCategory("org.khronos.openxr.intent.category.IMMERSIVE_HMD");
            intent.putExtra("server", server).putExtra("device", device).putExtra("item", item).putExtra("playback", data.toString()).putExtra("passthrough", passthrough.isChecked());
            startActivity(intent);
        }));
        else root.addView(label("Immersive playback is unavailable for this format or missing video dimensions."));
        root.addView(button("2D preview", v -> { try { playback(item, data); } catch (Exception e) { showStatus("Could not start playback"); } }));
        root.addView(button("Back", v -> browse(0)));
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
    @Override protected void onDestroy() { generation++; io.shutdown(); super.onDestroy(); }
}
