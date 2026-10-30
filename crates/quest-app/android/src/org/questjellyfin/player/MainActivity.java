package org.questjellyfin.player;

import android.app.Activity;
import android.app.AlertDialog;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.content.SharedPreferences;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.util.Base64;
import android.util.LruCache;
import android.media.MediaPlayer;
import android.net.Uri;
import android.view.Gravity;
import android.view.View;
import android.view.inputmethod.InputMethodManager;
import android.widget.*;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.HashMap;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Native Jellyfin browsing and playback, sized for Quest hand and controller input. */
public class MainActivity extends Activity {
    static { System.loadLibrary("quest_app"); }
    static native String nativeRequest(String json);
    private static final int BG = Color.rgb(16, 17, 24), CARD = Color.rgb(30, 32, 44), ACCENT = Color.rgb(0, 164, 220);
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final ExecutorService images = Executors.newFixedThreadPool(3);
    private final Handler ui = new Handler(Looper.getMainLooper());
    private final LruCache<String, Bitmap> posterCache = new LruCache<String, Bitmap>(24 * 1024 * 1024) {
        @Override protected int sizeOf(String key, Bitmap image) { return image.getAllocationByteCount(); }
    };
    private SharedPreferences prefs;
    private LinearLayout root, body;
    private TextView status;
    private ProgressBar spinner;
    private Button retryButton;
    private Runnable retryAction;
    private String server, device, userName = "", pairingSecret;
    private String activeItem, activeSource, activeSession, activeMethod;
    private VideoView video;
    private MediaPlayer panelPlayer;
    private FrameLayout videoLoading;
    private boolean started;
    private volatile int generation;
    private int lastPosition, currentStart;
    private String searchTerm = "", category = "all", parentId = "", libraryName = "Library";
    private boolean signedIn;
    private Runnable backAction;

    interface Done { void apply(JSONObject data) throws Exception; }
    interface Failed { void apply(String error); }
    private JSONObject value(String key, Object val) {
        JSONObject o = new JSONObject(); try { o.put(key, val); } catch (Exception ignored) {} return o;
    }
    private JSONObject with(JSONObject o, String key, Object val) {
        try { o.put(key, val); } catch (Exception ignored) {} return o;
    }
    private JSONObject envelope(String action, JSONObject extra) throws Exception {
        JSONObject req = extra == null ? new JSONObject() : extra;
        return req.put("action", action).put("server", server).put("device", device)
            .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json");
    }
    private void request(String action, JSONObject extra, Done done) { request(action, extra, done, this::error); }
    private void request(String action, JSONObject extra, Done done, Failed failed) {
        final String targetServer = server;
        final int current = generation;
        try {
            String raw = envelope(action, extra).toString();
            io.execute(() -> {
                JSONObject reply;
                try { reply = new JSONObject(nativeRequest(raw)); }
                catch (Exception e) { reply = value("ok", false); with(reply, "error", "Could not read the server response."); }
                final JSONObject response = reply;
                ui.post(() -> {
                    if (isFinishing() || isDestroyed() || current != generation || !targetServer.equals(server)) return;
                    try {
                        if (response.optBoolean("ok")) done.apply(response.getJSONObject("data"));
                        else failed.apply(response.optString("error", "The request failed."));
                    } catch (Exception e) { failed.apply("Could not display the server response."); }
                });
            });
        } catch (Exception e) { failed.apply("Could not prepare the request."); }
    }
    private int dp(float n) { return Math.round(n * getResources().getDisplayMetrics().density); }
    private GradientDrawable background(int color, int stroke) {
        GradientDrawable d = new GradientDrawable(); d.setColor(color); d.setCornerRadius(dp(9));
        if (stroke != 0) d.setStroke(dp(2), stroke); return d;
    }
    private TextView label(String text, int size) {
        TextView t = new TextView(this); t.setText(text); t.setTextSize(size); t.setTextColor(Color.WHITE); return t;
    }
    private TextView heading(String text) {
        TextView t = label(text, 24); t.setTypeface(null, Typeface.BOLD); t.setPadding(0, dp(18), 0, dp(12)); return t;
    }
    private Button button(String text, View.OnClickListener click) {
        Button b = new Button(this); b.setText(text); b.setTextSize(16); b.setTextColor(Color.WHITE); b.setAllCaps(false);
        b.setMinHeight(dp(48)); b.setPadding(dp(18), dp(4), dp(18), dp(4)); b.setBackground(background(CARD, 0));
        b.setOnFocusChangeListener((v, focused) -> v.setBackground(background(CARD, focused ? ACCENT : 0)));
        b.setOnClickListener(click); return b;
    }
    private void addButton(LinearLayout parent, String title, Runnable action) {
        Button b = button(title, v -> action.run()); LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-2, -2);
        lp.setMargins(0, dp(4), dp(10), dp(4)); parent.addView(b, lp);
    }
    private void page(String title, boolean navigation) {
        generation++; pairingSecret = null;
        root = new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL); root.setPadding(dp(24), dp(14), dp(24), dp(14)); root.setBackgroundColor(BG);
        LinearLayout header = new LinearLayout(this); header.setGravity(Gravity.CENTER_VERTICAL);
        TextView brand = label("Jellyfin", 28); brand.setTextColor(ACCENT); brand.setTypeface(null, Typeface.BOLD);
        header.addView(brand); TextView subtitle = label("  /  " + title, 20); header.addView(subtitle, new LinearLayout.LayoutParams(0, -2, 1));
        TextView quest = label("QUEST", 13); quest.setTextColor(Color.LTGRAY); header.addView(quest); root.addView(header);
        if (navigation) {
            HorizontalScrollView navScroll = new HorizontalScrollView(this); navScroll.setHorizontalScrollBarEnabled(false);
            LinearLayout nav = new LinearLayout(this); navScroll.addView(nav);
            addButton(nav, "Home", this::dashboard);
            addButton(nav, "Movies", () -> openCategory("movies")); addButton(nav, "Shows", () -> openCategory("shows"));
            addButton(nav, "Favorites", () -> openCategory("favorites")); addButton(nav, "Search", () -> openCategory("all"));
            addButton(nav, "Settings", this::settings); root.addView(navScroll);
        }
        LinearLayout state = new LinearLayout(this); state.setGravity(Gravity.CENTER_VERTICAL);
        spinner = new ProgressBar(this); state.addView(spinner, new LinearLayout.LayoutParams(dp(28), dp(28)));
        status = label("", 16); status.setPadding(dp(10), dp(7), dp(10), dp(7)); state.addView(status, new LinearLayout.LayoutParams(0, -2, 1));
        retryButton = button("Retry", v -> { if (retryAction != null) retryAction.run(); }); state.addView(retryButton); root.addView(state);
        spinner.setVisibility(View.GONE); retryButton.setVisibility(View.GONE);
        body = new LinearLayout(this); body.setOrientation(LinearLayout.VERTICAL); root.addView(body, new LinearLayout.LayoutParams(-1, 0, 1));
        setContentView(root); backAction = navigation ? this::dashboard : this::home; retryAction = null;
    }
    private LinearLayout scrollBody() {
        ScrollView scroll = new ScrollView(this); scroll.setFillViewport(true);
        LinearLayout content = new LinearLayout(this); content.setOrientation(LinearLayout.VERTICAL); scroll.addView(content);
        body.addView(scroll, new LinearLayout.LayoutParams(-1, -1)); return content;
    }
    private void loading(String text) { spinner.setVisibility(View.VISIBLE); retryButton.setVisibility(View.GONE); status.setTextColor(Color.LTGRAY); status.setText(text); }
    private void ready(String text) { spinner.setVisibility(View.GONE); retryButton.setVisibility(View.GONE); status.setTextColor(Color.LTGRAY); status.setText(text); }
    private void error(String text) { spinner.setVisibility(View.GONE); status.setTextColor(Color.rgb(255, 160, 145)); status.setText(text); retryButton.setVisibility(retryAction == null ? View.GONE : View.VISIBLE); }
    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved); prefs = getSharedPreferences("app", MODE_PRIVATE); device = prefs.getString("device", null);
        if (device == null) { device = UUID.randomUUID().toString(); prefs.edit().putString("device", device).apply(); }
        server = prefs.getString("server", "");
        if (server.isEmpty()) home(); else connect();
    }
    private void home() {
        page("Connect to your server", false); backAction = null;
        LinearLayout content = scrollBody(); content.addView(heading("Your library. Your cinema."));
        content.addView(label("Connect to Jellyfin to browse your movies and shows on Quest.", 19));
        EditText address = new EditText(this); address.setSingleLine(); address.setTextColor(Color.WHITE); address.setHintTextColor(Color.LTGRAY);
        address.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_URI);
        address.setHint("https://your-jellyfin-server"); address.setText(server); content.addView(address);
        addButton(content, "Connect", () -> {
            server = address.getText().toString().trim(); if (server.isEmpty()) { error("Enter your Jellyfin server URL."); return; }
            ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(address.getWindowToken(), 0);
            connect();
        });
        content.addView(label("Authorize a Quick Connect code from a signed-in Jellyfin app.\nRegular 2D movies also play on the immersive cinema screen.", 17));
    }
    private void connect() {
        page("Connecting", false); loading("Restoring your Jellyfin session…"); retryAction = this::connect;
        addButton(body, "Change server", this::home);
        request("status", null, data -> {
            prefs.edit().putString("server", server).apply(); signedIn = data.optBoolean("signed_in"); userName = data.optString("user", "");
            if (signedIn) dashboard(); else pair();
        });
    }
    private void pair() {
        page("Quick Connect", false); int current = generation; LinearLayout content = scrollBody();
        TextView code = heading("Requesting a code…"); code.setTextSize(40); content.addView(code);
        content.addView(label("In a signed-in Jellyfin app, open Settings → Quick Connect and enter this code.", 21));
        addButton(content, "Change server", this::home); loading("Contacting Jellyfin…"); retryAction = this::pair;
        request("initiate", null, data -> {
            pairingSecret = data.getString("secret"); code.setText(data.getString("code")); loading("Waiting for authorization…"); poll(current);
        });
    }
    private void poll(int current) {
        ui.postDelayed(() -> {
            if (generation != current || pairingSecret == null) return;
            request("poll", value("secret", pairingSecret), data -> {
                if (data.optBoolean("authorized")) { signedIn = true; dashboard(); } else poll(current);
            });
        }, 3000);
    }
    private ImageView poster(JSONObject item, int current) {
        ImageView image = new ImageView(this); image.setScaleType(ImageView.ScaleType.CENTER_CROP);
        image.setBackground(background(CARD, 0)); image.setContentDescription("Poster for " + item.optString("name", "title"));
        String id = item.optString("image_id", item.optString("id")); if (id.isEmpty() || "null".equals(id)) id = item.optString("id");
        String key = server + "|" + id + "|" + item.optString("image_tag", ""); Bitmap cached = posterCache.get(key);
        if (cached != null) { image.setImageBitmap(cached); return image; }
        final String imageId = id;
        try {
            String raw = envelope("image", value("item", imageId)).toString();
            images.execute(() -> {
                if (generation != current) return;
                Bitmap bitmap = null;
                try {
                    JSONObject reply = new JSONObject(nativeRequest(raw));
                    if (reply.optBoolean("ok") && !reply.getJSONObject("data").isNull("image")) {
                        byte[] bytes = Base64.decode(reply.getJSONObject("data").getString("image"), Base64.DEFAULT);
                        bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
                    }
                } catch (Exception ignored) { /* Keep a labeled placeholder if this title has no artwork. */ }
                final Bitmap loaded = bitmap;
                ui.post(() -> {
                    if (isDestroyed() || generation != current) return;
                    if (loaded != null) { posterCache.put(key, loaded); image.setImageBitmap(loaded); }
                    else { image.setImageResource(android.R.drawable.ic_menu_gallery); image.setScaleType(ImageView.ScaleType.CENTER); image.setAlpha(0.55f); }
                });
            });
        } catch (Exception ignored) {}
        return image;
    }
    private String itemCaption(JSONObject item) {
        String name = item.optString("name", "Untitled");
        if ("Episode".equals(item.optString("type"))) {
            String series = item.optString("series", "");
            return (series.isEmpty() || series.equals("null") ? "" : series + " · ") + "S" + item.optInt("season") + " E" + item.optInt("episode") + "\n" + name;
        }
        return name;
    }
    private LinearLayout card(JSONObject item, int width, int current) {
        LinearLayout card = new LinearLayout(this); card.setOrientation(LinearLayout.VERTICAL); card.setPadding(dp(7), dp(7), dp(7), dp(10));
        card.setBackground(background(CARD, 0)); card.setFocusable(true); card.setClickable(true);
        card.setContentDescription("Open " + itemCaption(item));
        card.setOnFocusChangeListener((v, focused) -> v.setBackground(background(CARD, focused ? ACCENT : 0)));
        card.setOnClickListener(v -> { if ("Series".equals(item.optString("type"))) openSeries(item); else details(item.optString("id")); });
        card.addView(poster(item, current), new LinearLayout.LayoutParams(-1, dp(width * 1.48f)));
        long runtime = item.optLong("runtime"), resume = item.optLong("resume");
        if (resume > 0 && runtime > 0) {
            ProgressBar watched = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal); watched.setMax(1000);
            watched.setProgress((int)Math.min(1000, resume * 1000.0 / runtime)); card.addView(watched, new LinearLayout.LayoutParams(-1, dp(5)));
        }
        TextView title = label(itemCaption(item), 17); title.setMaxLines(2); title.setPadding(0, dp(7), 0, 0); card.addView(title);
        String meta = item.optInt("year") > 0 ? String.valueOf(item.optInt("year")) : "";
        if (item.optBoolean("played")) meta += "  ✓"; if (item.optBoolean("favorite")) meta += "  ♥";
        TextView info = label(meta, 14); info.setTextColor(Color.LTGRAY); card.addView(info); return card;
    }
    private void rail(LinearLayout parent, JSONArray items, int current) throws Exception {
        if (items.length() == 0) { parent.addView(label("Nothing here yet.", 17)); return; }
        HorizontalScrollView scroll = new HorizontalScrollView(this); LinearLayout row = new LinearLayout(this); scroll.addView(row);
        for (int i = 0; i < items.length(); i++) {
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(dp(185), -2); lp.setMargins(0, 0, dp(14), dp(5));
            row.addView(card(items.getJSONObject(i), 171, current), lp);
        }
        parent.addView(scroll);
    }
    private void dashboard() {
        page("Home", true); backAction = null; retryAction = this::dashboard;
        LinearLayout content = scrollBody(); ready(userName.isEmpty() ? "Welcome to Jellyfin on Quest" : "Welcome, " + userName);
        content.addView(heading("My Media")); HorizontalScrollView librariesScroll = new HorizontalScrollView(this); LinearLayout libraries = new LinearLayout(this); librariesScroll.addView(libraries); content.addView(librariesScroll);
        ProgressBar libraryLoading = new ProgressBar(this); libraries.addView(libraryLoading, new LinearLayout.LayoutParams(dp(30), dp(30)));
        request("libraries", null, data -> {
            libraries.removeAllViews(); JSONArray items = data.getJSONArray("items");
            for (int i = 0; i < items.length(); i++) {
                JSONObject item = items.getJSONObject(i);
                addButton(libraries, item.optString("name"), () -> { category = "all"; parentId = item.optString("id"); libraryName = item.optString("name"); searchTerm = ""; browse(0); });
            }
            if (items.length() == 0) libraries.addView(label("No libraries available for this account.", 17));
        }, message -> { libraries.removeAllViews(); error(message); });
        content.addView(heading("Continue Watching")); LinearLayout resume = new LinearLayout(this); resume.setOrientation(1); content.addView(resume);
        ProgressBar resumeLoading = new ProgressBar(this); resume.addView(resumeLoading, new LinearLayout.LayoutParams(dp(30), dp(30)));
        int current = generation;
        request("resume", null, data -> { resume.removeAllViews(); rail(resume, data.getJSONArray("items"), current); }, message -> { resume.removeAllViews(); resume.addView(label("Continue Watching unavailable. Use Retry to reload.", 17)); error(message); });
        content.addView(heading("Recently Added")); LinearLayout recent = new LinearLayout(this); recent.setOrientation(1); content.addView(recent);
        ProgressBar latestLoading = new ProgressBar(this); recent.addView(latestLoading, new LinearLayout.LayoutParams(dp(30), dp(30)));
        request("browse", with(value("sort", "latest"), "limit", 16), data -> { recent.removeAllViews(); rail(recent, data.getJSONArray("items"), current); }, message -> { recent.removeAllViews(); error(message); });
    }
    private void openCategory(String kind) { category = kind; parentId = ""; libraryName = "Library"; searchTerm = ""; browse(0); }
    private void openSeries(JSONObject series) { category = "episodes"; parentId = series.optString("id"); libraryName = series.optString("name"); searchTerm = ""; browse(0); }
    private void browse(int start) {
        currentStart = start;
        String title = !parentId.isEmpty() ? libraryName : category.equals("movies") ? "Movies" : category.equals("shows") ? "Shows" : category.equals("favorites") ? "Favorites" : "Library";
        page(title, true); retryAction = () -> browse(start); int current = generation;
        LinearLayout searchRow = new LinearLayout(this); EditText search = new EditText(this); search.setSingleLine(); search.setTextColor(Color.WHITE); search.setHintTextColor(Color.LTGRAY);
        search.setHint("Search " + title); search.setText(searchTerm); searchRow.addView(search, new LinearLayout.LayoutParams(0, -2, 1));
        addButton(searchRow, "Search", () -> {
            searchTerm = search.getText().toString().trim(); ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(search.getWindowToken(), 0); browse(0);
        });
        body.addView(searchRow); LinearLayout list = scrollBody(); loading("Loading titles…");
        JSONObject query = with(with(with(value("start", start), "search", searchTerm), "parent", parentId), "kind", category);
        with(query, "sort", prefs.getString("sort", "name")); if (category.equals("favorites")) with(query, "favorite", true);
        request("browse", query, data -> {
            JSONArray items = data.getJSONArray("items"); ready(items.length() == 0 ? "No matching titles. Try another search or library." : "Select a title to see details and playback options");
            int columns = QuestPreferences.columns(prefs.getInt("columns", 5));
            int screenDp = (int)(getResources().getDisplayMetrics().widthPixels / getResources().getDisplayMetrics().density);
            columns = Math.min(columns, Math.max(2, screenDp / 180));
            int cardWidth = Math.max(120, (screenDp - 48) / columns - 26); LinearLayout row = null;
            for (int i = 0; i < items.length(); i++) {
                if (i % columns == 0) { row = new LinearLayout(this); list.addView(row); }
                LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, -2, 1); lp.setMargins(dp(5), dp(5), dp(5), dp(10));
                row.addView(card(items.getJSONObject(i), cardWidth, current), lp);
            }
            if (row != null && items.length() % columns != 0) for (int i = items.length() % columns; i < columns; i++) row.addView(new View(this), new LinearLayout.LayoutParams(0, 1, 1));
            LinearLayout pages = new LinearLayout(this); list.addView(pages);
            if (start > 0) addButton(pages, "Previous page", () -> browse(Math.max(0, start - 50)));
            if (!data.isNull("next")) { int next = data.getInt("next"); addButton(pages, "Next page", () -> browse(next)); }
        });
    }
    private String metadata(JSONObject item) {
        String year = item.optInt("year") > 0 ? String.valueOf(item.optInt("year")) : "";
        String rating = item.optString("rating", ""); if (rating.equals("null")) rating = "";
        long minutes = item.optLong("runtime") / 600000000L;
        return year + (rating.isEmpty() ? "" : "  ·  " + rating) + (minutes > 0 ? "  ·  " + minutes + " min" : "");
    }
    private void details(String id) {
        page("Title details", true); backAction = () -> browse(currentStart); retryAction = () -> details(id); loading("Loading title details…");
        LinearLayout content = scrollBody();
        request("details", value("item", id), item -> {
            ready(metadata(item)); LinearLayout row = new LinearLayout(this); content.addView(row);
            row.addView(poster(item, generation), new LinearLayout.LayoutParams(dp(235), dp(350)));
            LinearLayout info = new LinearLayout(this); info.setOrientation(1); info.setPadding(dp(24), 0, 0, 0); row.addView(info, new LinearLayout.LayoutParams(0, -2, 1));
            info.addView(heading(itemCaption(item))); String overview = item.optString("overview", "");
            if (!overview.equals("null") && !overview.isEmpty()) info.addView(label(overview, 19));
            long resume = item.optLong("resume");
            if (resume > 0) addButton(info, "Resume from " + position(resume / 10000L), () -> prepare(id, true));
            addButton(info, resume > 0 ? "Play from beginning" : "Play", () -> prepare(id, false));
            Button favorite = button(item.optBoolean("favorite") ? "♥ Remove favorite" : "♡ Add favorite", null);
            favorite.setOnClickListener(v -> { favorite.setEnabled(false); boolean next = !item.optBoolean("favorite"); request("favorite", with(value("item", id), "favorite", next), d -> { with(item, "favorite", next); favorite.setText(next ? "♥ Remove favorite" : "♡ Add favorite"); favorite.setEnabled(true); }, message -> { favorite.setEnabled(true); error(message); }); });
            info.addView(favorite); addButton(content, "Back to library", () -> browse(currentStart));
        });
    }
    private String position(long ms) { return String.format(java.util.Locale.ROOT, "%d:%02d", ms / 60000, (ms / 1000) % 60); }
    private void prepare(String item, boolean resume) {
        page("Opening video", true); backAction = () -> details(item); retryAction = () -> prepare(item, resume);
        loading("Requesting playback from Jellyfin…"); addButton(body, "Cancel", () -> details(item));
        JSONObject preparation = with(value("item", item), "allow_transcoding", prefs.getBoolean("allow_transcoding", true));
        String quality = prefs.getString("quality", "original");
        if (!quality.equals("original")) { try { with(preparation, "max_bitrate", Long.parseLong(quality)); } catch (NumberFormatException ignored) {} }
        request("prepare", preparation, data -> {
            if (!resume || !prefs.getBoolean("resume", true)) with(data, "resume", 0);
            choosePlayback(item, data);
        });
    }
    private void choosePlayback(String item, JSONObject data) {
        page(data.optString("title", "Playback"), true); backAction = () -> details(item); LinearLayout content = scrollBody();
        String format = data.optString("format", "Mono"), projection = data.optString("projection", "Flat");
        ready(data.optString("method") + "  ·  " + format + "  ·  " + data.optInt("width") + " × " + data.optInt("height"));
        boolean supported = projection.equals("Flat") && !format.equals("Mvc") && !format.equals("MvHevc") && !format.equals("Unspecified3d") && data.optInt("width") > 0 && data.optInt("height") > 0;
        content.addView(heading("Where would you like to watch?"));
        if (supported) addButton(content, "Watch in immersive cinema", () -> launchCinema(item, data));
        addButton(content, "Watch in Quest panel", () -> { try { playback(item, data); } catch (Exception e) { error("Could not start playback."); } });
        content.addView(label("Cinema: trigger pauses / resumes · thumbstick left / right seeks 10 seconds · grip changes audio · thumbstick up recenters the screen.", 18));
        if (!data.optString("method").equals("DirectPlay")) content.addView(label("Jellyfin selected server conversion. Resolution, bitrate or HDR may change.", 17));
        if (!supported) content.addView(label("This source's projection or stereo packing is not yet supported in immersive cinema.", 18));
        addButton(content, "Playback settings", this::settings); addButton(content, "Back to title", () -> details(item));
        String preferred = prefs.getString("player", "ask");
        if (preferred.equals("cinema") && supported) launchCinema(item, data);
        else if (preferred.equals("panel")) { try { playback(item, data); } catch (Exception e) { error("Could not start playback."); } }
    }
    private void launchCinema(String item, JSONObject data) {
        Intent intent = new Intent(this, VrActivity.class); intent.addCategory("com.oculus.intent.category.VR"); intent.addCategory("org.khronos.openxr.intent.category.IMMERSIVE_HMD");
        intent.putExtra("server", server).putExtra("device", device).putExtra("item", item).putExtra("playback", data.toString())
            .putExtra("passthrough", prefs.getBoolean("passthrough", false))
            .putExtra("screen_width", QuestPreferences.screenWidth(prefs.getFloat("screen_width", QuestPreferences.DEFAULT_WIDTH)))
            .putExtra("screen_distance", QuestPreferences.screenDistance(prefs.getFloat("screen_distance", QuestPreferences.DEFAULT_DISTANCE)));
        loading("Starting immersive cinema…"); startActivityForResult(intent, 1);
    }
    @Override protected void onActivityResult(int requestCode, int resultCode, Intent result) {
        super.onActivityResult(requestCode, resultCode, result);
        if (requestCode == 1) {
            String error = result == null ? null : result.getStringExtra("error");
            if (error == null) ready("Playback closed."); else error(error);
        }
    }
    private JSONObject playbackData(long ticks) {
        return with(with(with(with(value("item", activeItem), "source", activeSource), "session", activeSession == null ? JSONObject.NULL : activeSession), "ticks", ticks), "method", activeMethod);
    }
    private void playback(String item, JSONObject data) throws Exception {
        activeItem = item; activeMethod = data.optString("method", "DirectPlay"); activeSource = data.getString("source"); activeSession = data.isNull("session") ? null : data.getString("session");
        long resumeMs = data.optLong("resume") / 10000L; page(data.optString("title", "Now playing"), false); int current = generation;
        backAction = () -> { stopPlayback(); details(item); }; retryAction = () -> { stopPlayback(); try { playback(item, data); } catch (Exception e) { error("Could not reopen video."); } };
        LinearLayout actions = new LinearLayout(this); addButton(actions, "Back to title", backAction); addButton(actions, "Audio", () -> tracks(MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO)); addButton(actions, "Subtitles", () -> tracks(MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_SUBTITLE)); body.addView(actions);
        FrameLayout frame = new FrameLayout(this); body.addView(frame, new LinearLayout.LayoutParams(-1, 0, 1));
        video = new VideoView(this); FrameLayout.LayoutParams videoLp = new FrameLayout.LayoutParams(-1, -1, Gravity.CENTER); frame.addView(video, videoLp);
        videoLoading = new FrameLayout(this); videoLoading.setBackgroundColor(0xB0101118);
        LinearLayout load = new LinearLayout(this); load.setOrientation(1); load.setGravity(Gravity.CENTER); load.addView(new ProgressBar(this)); load.addView(label("Opening video…", 22));
        videoLoading.addView(load, new FrameLayout.LayoutParams(-1, -1)); frame.addView(videoLoading, new FrameLayout.LayoutParams(-1, -1));
        MediaController controls = new MediaController(this); controls.setAnchorView(video); video.setMediaController(controls);
        video.setOnPreparedListener(mp -> {
            if (current != generation || video == null) return; panelPlayer = mp;
            if (resumeMs > 0 && resumeMs <= Integer.MAX_VALUE) video.seekTo((int)resumeMs); video.start();
        });
        video.setOnInfoListener((mp, what, extra) -> {
            if (current != generation || video == null) return false;
            if (what == MediaPlayer.MEDIA_INFO_VIDEO_RENDERING_START) {
                videoLoading.setVisibility(View.GONE); ready("Playing");
                if (!started) { started = true; lastPosition = video.getCurrentPosition(); request("start", playbackData(lastPosition * 10000L), d -> {}, message -> {}); progress(current); }
            } else if (what == MediaPlayer.MEDIA_INFO_BUFFERING_START) { videoLoading.setVisibility(View.VISIBLE); loading("Buffering video…"); }
            else if (what == MediaPlayer.MEDIA_INFO_BUFFERING_END && started) { videoLoading.setVisibility(View.GONE); ready("Playing"); }
            return false;
        });
        video.setOnCompletionListener(mp -> { stopPlayback(); details(item); });
        video.setOnErrorListener((mp, what, extra) -> { stopPlayback(); if (videoLoading != null) videoLoading.setVisibility(View.GONE); error("This device could not play the selected source (" + what + ", " + extra + ")."); return true; });
        loading("Opening video…"); HashMap<String, String> headers = new HashMap<>(); headers.put("Authorization", data.getString("authorization")); headers.put("android-allow-cross-domain-redirect", "0");
        video.setVideoURI(Uri.parse(data.getString("url")), headers);
        ui.postDelayed(() -> { if (generation == current && video != null && !started) error("Video is taking longer than expected. You can retry or go back."); }, 20000);
    }
    private void tracks(int type) {
        if (panelPlayer == null) { error("Wait for video to load before selecting tracks."); return; }
        try {
            MediaPlayer.TrackInfo[] tracks = panelPlayer.getTrackInfo(); java.util.ArrayList<Integer> ids = new java.util.ArrayList<>(); java.util.ArrayList<String> names = new java.util.ArrayList<>();
            if (type == MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_SUBTITLE) { ids.add(-1); names.add("Off"); }
            int selected = panelPlayer.getSelectedTrack(type), checked = 0;
            for (int i = 0; i < tracks.length; i++) if (tracks[i].getTrackType() == type || (type == MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_SUBTITLE && tracks[i].getTrackType() == MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_TIMEDTEXT)) {
                if (i == selected) checked = ids.size(); ids.add(i); names.add("Track " + (ids.size()) + " · " + tracks[i].getLanguage());
            }
            if (ids.isEmpty()) { ready("No selectable tracks in this stream."); return; }
            new AlertDialog.Builder(this).setTitle(type == MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO ? "Audio" : "Subtitles")
                .setSingleChoiceItems(names.toArray(new String[0]), checked, (dialog, which) -> {
                    try { if (panelPlayer != null) { if (ids.get(which) < 0 && selected >= 0) panelPlayer.deselectTrack(selected); else if (ids.get(which) >= 0) panelPlayer.selectTrack(ids.get(which)); } }
                    catch (RuntimeException e) { error("This track could not be selected."); } dialog.dismiss();
                }).setNegativeButton("Cancel", null).show();
        } catch (RuntimeException e) { error("Track selection is unavailable for this source."); }
    }
    private void progress(int current) {
        ui.postDelayed(() -> {
            if (current != generation || video == null || !started) return;
            lastPosition = video.getCurrentPosition(); request("progress", with(playbackData(lastPosition * 10000L), "paused", !video.isPlaying()), d -> {}, message -> {}); progress(current);
        }, 10000);
    }
    private void stopPlayback() {
        if (video != null) { if (started) lastPosition = video.getCurrentPosition(); video.stopPlayback(); video = null; panelPlayer = null; }
        if (started) { request("stop", playbackData(lastPosition * 10000L), d -> {}, message -> {}); started = false; }
    }
    private void choice(LinearLayout content, String title, String key, String[] labels, String[] values, String fallback) {
        content.addView(heading(title)); RadioGroup group = new RadioGroup(this); String saved = prefs.getString(key, fallback);
        for (int i = 0; i < labels.length; i++) { RadioButton b = new RadioButton(this); b.setText(labels[i]); b.setTextColor(Color.WHITE); b.setTextSize(18); b.setMinHeight(dp(48)); final String val = values[i]; b.setChecked(saved.equals(val)); b.setOnClickListener(v -> prefs.edit().putString(key, val).apply()); group.addView(b); }
        content.addView(group);
    }
    private void toggle(LinearLayout content, String label, String key, boolean fallback) {
        Switch sw = new Switch(this); sw.setText(label); sw.setTextSize(19); sw.setTextColor(Color.WHITE); sw.setMinHeight(dp(56)); sw.setChecked(prefs.getBoolean(key, fallback)); sw.setOnCheckedChangeListener((b, checked) -> prefs.edit().putBoolean(key, checked).apply()); content.addView(sw);
    }
    private void slider(LinearLayout content, String title, String key, float min, float max, float saved) {
        TextView label = heading(title + String.format(java.util.Locale.ROOT, " · %.1f m", saved)); content.addView(label);
        SeekBar seek = new SeekBar(this); seek.setMax(Math.round((max - min) * 10)); seek.setProgress(Math.round((saved - min) * 10)); seek.setMinimumHeight(dp(48));
        seek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            public void onProgressChanged(SeekBar bar, int n, boolean user) { float val = min + n / 10f; label.setText(title + String.format(java.util.Locale.ROOT, " · %.1f m", val)); if (user) prefs.edit().putFloat(key, val).apply(); }
            public void onStartTrackingTouch(SeekBar bar) {} public void onStopTrackingTouch(SeekBar bar) {}
        }); content.addView(seek);
    }
    private void settings() {
        page("Settings", signedIn); LinearLayout content = scrollBody(); ready("Changes save automatically and apply the next time you open a video.");
        choice(content, "Default player", "player", new String[]{"Ask each time", "Immersive cinema", "Quest panel"}, new String[]{"ask", "cinema", "panel"}, "ask");
        choice(content, "Streaming bitrate ceiling", "quality", new String[]{"Original / device default (up to 120 Mbps)", "80 Mbps", "40 Mbps", "20 Mbps"}, new String[]{"original", "80000000", "40000000", "20000000"}, "original");
        toggle(content, "Allow Jellyfin server transcoding", "allow_transcoding", true);
        content.addView(label("A lower ceiling can trigger conversion and reduce picture quality. Disable transcoding to require direct play or remux.", 17));
        content.addView(heading("Cinema")); toggle(content, "Show the room with passthrough", "passthrough", false); toggle(content, "Resume from Jellyfin's saved position", "resume", true);
        slider(content, "Screen width", "screen_width", 1.5f, 6f, QuestPreferences.screenWidth(prefs.getFloat("screen_width", QuestPreferences.DEFAULT_WIDTH)));
        slider(content, "Viewing distance", "screen_distance", 1f, 5f, QuestPreferences.screenDistance(prefs.getFloat("screen_distance", QuestPreferences.DEFAULT_DISTANCE)));
        content.addView(label("The cinema opens in front of your current viewing direction. Push the thumbstick up to recenter it.", 17));
        choice(content, "Library sorting", "sort", new String[]{"Title", "Recently added"}, new String[]{"name", "latest"}, "name");
        content.addView(heading("Poster size")); RadioGroup columns = new RadioGroup(this); int count = QuestPreferences.columns(prefs.getInt("columns", 5));
        for (int n = 3; n <= 6; n++) { final int value = n; RadioButton b = new RadioButton(this); b.setText(n + " columns"); b.setTextSize(18); b.setTextColor(Color.WHITE); b.setMinHeight(dp(48)); b.setChecked(n == count); b.setOnClickListener(v -> prefs.edit().putInt("columns", value).apply()); columns.addView(b); } content.addView(columns);
        content.addView(heading("Account & server")); content.addView(label((userName.isEmpty() ? "Jellyfin account" : userName) + "\n" + server, 18));
        addButton(content, "Change server", this::home);
        if (signedIn) addButton(content, "Sign out", () -> { loading("Signing out…"); request("signout", null, d -> { signedIn = false; userName = ""; posterCache.evictAll(); home(); }); });
        addButton(content, "Clear cached artwork", () -> { posterCache.evictAll(); ready("Artwork cache cleared."); });
        content.addView(heading("About")); content.addView(label("Jellyfin 3D for Quest · 0.1.0\nIndependent native Jellyfin client using DeskQuest XR components.\nStereo cinema supports mono, SBS and over-under. MVC, MV-HEVC and immersive subtitles are still unavailable.", 16));
        addButton(content, "Back", signedIn ? this::dashboard : this::home);
    }
    @Override public void onBackPressed() { if (backAction != null) backAction.run(); else super.onBackPressed(); }
    @Override protected void onStop() { super.onStop(); if (video != null) { String item = activeItem; stopPlayback(); details(item); } }
    @Override protected void onDestroy() { generation++; io.shutdown(); images.shutdownNow(); posterCache.evictAll(); super.onDestroy(); }
}
