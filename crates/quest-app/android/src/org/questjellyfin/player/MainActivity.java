package org.questjellyfin.player;

import android.app.Activity;
import android.app.AlertDialog;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ResultReceiver;
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
import android.view.MotionEvent;
import android.view.KeyEvent;
import android.os.SystemClock;
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
    private final ExecutorService reads = Executors.newFixedThreadPool(2);
    private final Handler ui = new Handler(Looper.getMainLooper());
    private final LruCache<String, Bitmap> posterCache = new LruCache<String, Bitmap>(24 * 1024 * 1024) {
        @Override protected int sizeOf(String key, Bitmap image) { return image.getAllocationByteCount(); }
    };
    private SharedPreferences prefs;
    private LinearLayout root, body, chrome;
    private final PlaybackChrome chromePolicy = new PlaybackChrome();
    private boolean playbackPage, touchingPlayer, hoveringChrome, threeDOnly, revealingChromeTouch;
    private MediaController panelControls;
    private Runnable filterReload = this::dashboard;
    private TextView status;
    private ProgressBar spinner;
    private Button retryButton;
    private Runnable retryAction;
    private String server, device, userName = "", userId = "", pairingSecret;
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
            .put("store", getFilesDir().getAbsolutePath() + "/jellyfin-session.json").put("user_id", userId);
    }
    private void request(String action, JSONObject extra, Done done) { request(action, extra, done, this::error); }
    private void request(String action, JSONObject extra, Done done, Failed failed) {
        final String targetServer = server;
        final int current = generation;
        try {
            String raw = envelope(action, extra).toString();
            boolean readOnly = action.equals("browse") || action.equals("details") || action.equals("prepare") || action.equals("libraries") || action.equals("resume") || action.equals("public_users");
            (readOnly ? reads : io).execute(() -> {
                if ((readOnly || action.equals("login") || action.equals("switch_user") || action.equals("poll")) && (current != generation || !targetServer.equals(server))) return;
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
        b.setOnFocusChangeListener((v, focused) -> v.setBackground(background(v.isSelected() ? Color.rgb(17,87,111) : CARD, focused || v.isSelected() ? ACCENT : 0)));
        b.setOnClickListener(click); return b;
    }
    private void addButton(LinearLayout parent, String title, Runnable action) {
        Button b = button(title, v -> action.run()); LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-2, -2);
        lp.setMargins(0, dp(4), dp(10), dp(4)); parent.addView(b, lp);
    }
    private void page(String title, boolean navigation) {
        generation++; pairingSecret = null; filterReload = this::dashboard; revealingChromeTouch = false; playbackPage = false; touchingPlayer = false; hoveringChrome = false; ui.removeCallbacks(chromeTick);
        root = new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL); root.setPadding(dp(24), dp(14), dp(24), dp(14)); root.setBackgroundColor(BG);
        chrome = new LinearLayout(this); chrome.setOrientation(LinearLayout.VERTICAL); root.addView(chrome);
        LinearLayout header = new LinearLayout(this); header.setGravity(Gravity.CENTER_VERTICAL); header.setPadding(0,0,0,dp(8));
        ImageView brand = new ImageView(this); brand.setContentDescription("Jellyfin service"); brand.setScaleType(ImageView.ScaleType.FIT_CENTER);
        brand.setImageResource(getResources().getIdentifier("jellyfin_service", "drawable", getPackageName()));
        header.addView(brand, new LinearLayout.LayoutParams(dp(48),dp(48)));
        TextView subtitle = label(title, 23); subtitle.setMaxLines(1); subtitle.setEllipsize(android.text.TextUtils.TruncateAt.END); subtitle.setTypeface(null,Typeface.BOLD); subtitle.setPadding(dp(14),0,dp(12),0);
        header.addView(subtitle, new LinearLayout.LayoutParams(0,-2,1));
        if (navigation && signedIn) {
            Button profile = button(userName.isEmpty() ? "Users" : userName + " ▾", v -> users());
            profile.setMaxLines(1); profile.setMaxEms(14); profile.setEllipsize(android.text.TextUtils.TruncateAt.END); profile.setContentDescription("Switch user"); header.addView(profile);
        }
        chrome.addView(header);
        if (navigation) {
            HorizontalScrollView navScroll = new HorizontalScrollView(this); navScroll.setHorizontalScrollBarEnabled(false);
            LinearLayout nav = new LinearLayout(this); navScroll.addView(nav);
            addButton(nav, "Home", this::dashboard);
            addButton(nav, "Movies", () -> openCategory("movies")); addButton(nav, "Shows", () -> openCategory("shows"));
            addButton(nav, "Favorites", () -> openCategory("favorites")); addButton(nav, "Search", () -> openCategory("all"));
            addButton(nav, "Settings", this::settings);
            Button stereo = button("3D only", v -> { threeDOnly = !threeDOnly; prefs.edit().putBoolean("3d_only",threeDOnly).apply(); filterReload.run(); });
            stereo.setSelected(threeDOnly); stereo.setContentDescription(threeDOnly ? "3D only enabled" : "Show only 3D media");
            stereo.setBackground(background(threeDOnly ? Color.rgb(17,87,111) : CARD,threeDOnly ? ACCENT : 0));
            LinearLayout navigationRow = new LinearLayout(this); navigationRow.setGravity(Gravity.CENTER_VERTICAL);
            navigationRow.addView(navScroll,new LinearLayout.LayoutParams(0,-2,1));
            LinearLayout.LayoutParams stereoLp = new LinearLayout.LayoutParams(-2,-2); stereoLp.setMargins(dp(8),dp(4),0,dp(4)); navigationRow.addView(stereo,stereoLp);
            chrome.addView(navigationRow);
        }
        LinearLayout state = new LinearLayout(this); state.setGravity(Gravity.CENTER_VERTICAL);
        spinner = new ProgressBar(this); state.addView(spinner, new LinearLayout.LayoutParams(dp(28), dp(28)));
        status = label("", 16); status.setPadding(dp(10), dp(7), dp(10), dp(7)); state.addView(status, new LinearLayout.LayoutParams(0, -2, 1));
        retryButton = button("Retry", v -> { if (retryAction != null) retryAction.run(); }); state.addView(retryButton); chrome.addView(state);
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
        server = prefs.getString("server", ""); threeDOnly = prefs.getBoolean("3d_only",false);
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
            prefs.edit().putString("server", server).apply(); signedIn = data.optBoolean("signed_in"); userName = data.optString("user", ""); userId = data.optString("user_id", "");
            if (signedIn) dashboard(); else users();
        });
    }
    private void pair() {
        page("Quick Connect", false); backAction = this::users; int current = generation; LinearLayout content = scrollBody();
        TextView code = heading("Requesting a code…"); code.setTextSize(40); content.addView(code);
        content.addView(label("In a signed-in Jellyfin app, open Settings → Quick Connect and enter this code.", 21));
        addButton(content, "Back to users", this::users); loading("Contacting Jellyfin…"); retryAction = this::pair;
        request("initiate", null, data -> {
            pairingSecret = data.getString("secret"); code.setText(data.getString("code")); loading("Waiting for authorization…"); poll(current);
        });
    }
    private void poll(int current) {
        ui.postDelayed(() -> {
            if (generation != current || pairingSecret == null) return;
            request("poll", value("secret", pairingSecret), data -> {
                if (data.optBoolean("authorized")) { resetAccountView(); connect(); } else poll(current);
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
            String raw = envelope("image", with(value("item", imageId), "fallback", item.optString("id"))).toString();
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
    private void resetAccountView() {
        posterCache.evictAll(); searchTerm = ""; category = "all"; parentId = ""; currentStart = 0; libraryName = "Library";
    }
    private void signedInAs(JSONObject data) {
        signedIn = true; userName = data.optString("user", ""); userId = data.optString("user_id", "");
        resetAccountView(); dashboard();
    }
    private void users() {
        stopPlayback(); page("Users", false); backAction = signedIn ? this::dashboard : this::home; retryAction = this::users;
        LinearLayout content = scrollBody(); ready("Choose an account on this server.");
        content.addView(heading("On this Quest")); LinearLayout saved = new LinearLayout(this); saved.setOrientation(1); content.addView(saved);
        request("profiles", null, data -> {
            JSONArray users = data.getJSONArray("users");
            if (users.length() == 0) saved.addView(label("No accounts saved yet.",17));
            for (int i=0;i<users.length();i++) {
                JSONObject account = users.getJSONObject(i); String name = account.optString("name");
                addButton(saved, name + (account.optBoolean("current") ? " · Current" : ""), () -> {
                    loading("Switching to " + name + "…");
                    request("switch_user", value("target_user",account.optString("id")), this::signedInAs, message -> {
                        login(name); error(message);
                    });
                });
            }
        });
        content.addView(heading("Server users")); LinearLayout available = new LinearLayout(this); available.setOrientation(1); content.addView(available);
        available.addView(label("Loading users…",17));
        request("public_users",null,data -> {
            available.removeAllViews(); JSONArray users = data.getJSONArray("users");
            if (users.length()==0) available.addView(label("This server hides its user list. Enter a username or use Quick Connect.",17));
            for (int i=0;i<users.length();i++) { String name=users.getJSONObject(i).optString("name"); addButton(available,name,() -> login(name)); }
        }, message -> { available.removeAllViews(); available.addView(label("User list unavailable. You can still sign in below.",17)); });
        addButton(content,"Sign in another account",() -> login(""));
        addButton(content,"Use Quick Connect",this::pair);
        addButton(content,"Change server",this::home);
        if (signedIn) addButton(content,"Back to library",this::dashboard);
    }
    private void login(String selectedName) {
        page("Sign in",false); backAction = this::users;
        LinearLayout content = scrollBody(); ready("Sign in to a Jellyfin account. Saved accounts can be switched without retyping a password.");
        content.addView(heading("Sign in to this server"));
        EditText name = new EditText(this); name.setSingleLine(); name.setHint("Username"); name.setContentDescription("Username"); name.setText(selectedName); name.setTextColor(Color.WHITE); name.setHintTextColor(Color.LTGRAY); content.addView(name);
        EditText password = new EditText(this); password.setSingleLine(); password.setHint("Password"); password.setContentDescription("Password"); password.setTextColor(Color.WHITE); password.setHintTextColor(Color.LTGRAY);
        password.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD); content.addView(password);
        Button submit = button("Sign in",null); content.addView(submit);
        submit.setOnClickListener(v -> {
            String username = name.getText().toString().trim(); if (username.isEmpty()) { error("Enter your username."); return; }
            JSONObject credentials = with(value("username",username),"password",password.getText().toString()); password.setText("");
            ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(password.getWindowToken(),0);
            submit.setEnabled(false); loading("Signing in…");
            request("login",credentials,this::signedInAs,message -> { submit.setEnabled(true); error(message); });
        });
        addButton(content,"Use Quick Connect",this::pair); addButton(content,"Back to users",this::users);
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
        card.setOnFocusChangeListener((v, focused) -> v.setBackground(background(v.isSelected() ? Color.rgb(17,87,111) : CARD, focused || v.isSelected() ? ACCENT : 0)));
        card.setOnClickListener(v -> { if ("Series".equals(item.optString("type"))) openSeries(item); else details(item.optString("id")); });
        card.addView(poster(item, current), new LinearLayout.LayoutParams(-1, dp(width * 1.48f)));
        long runtime = item.optLong("runtime"), resume = item.optLong("resume");
        if (resume > 0 && runtime > 0) {
            ProgressBar watched = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal); watched.setMax(1000);
            watched.setProgress((int)Math.min(1000, resume * 1000.0 / runtime)); card.addView(watched, new LinearLayout.LayoutParams(-1, dp(5)));
        }
        TextView title = label(itemCaption(item), 17); title.setMaxLines(2); title.setPadding(0, dp(7), 0, 0); card.addView(title);
        String meta = item.optInt("year") > 0 ? String.valueOf(item.optInt("year")) : "";
        String stereoFormat = item.optString("format", "");
        if (!stereoFormat.isEmpty() && !stereoFormat.equals("null") && !stereoFormat.equals("None")) meta = "3D  ·  " + meta;
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
        page("Home", true); filterReload = this::dashboard; backAction = null; retryAction = this::dashboard;
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
        request("resume", value("three_d_only",threeDOnly), data -> { resume.removeAllViews(); rail(resume, data.getJSONArray("items"), current); }, message -> { resume.removeAllViews(); resume.addView(label("Continue Watching unavailable. Use Retry to reload.", 17)); error(message); });
        content.addView(heading("Recently Added")); LinearLayout recent = new LinearLayout(this); recent.setOrientation(1); content.addView(recent);
        ProgressBar latestLoading = new ProgressBar(this); recent.addView(latestLoading, new LinearLayout.LayoutParams(dp(30), dp(30)));
        request("browse", with(with(value("sort", "latest"), "limit", 16),"three_d_only",threeDOnly), data -> { recent.removeAllViews(); rail(recent, data.getJSONArray("items"), current); }, message -> { recent.removeAllViews(); error(message); });
    }
    private void openCategory(String kind) { category = kind; parentId = ""; libraryName = "Library"; searchTerm = ""; browse(0); }
    private void openSeries(JSONObject series) { category = "episodes"; parentId = series.optString("id"); libraryName = series.optString("name"); searchTerm = ""; browse(0); }
    private void browse(int start) {
        currentStart = start;
        String title = !parentId.isEmpty() ? libraryName : category.equals("movies") ? "Movies" : category.equals("shows") ? "Shows" : category.equals("favorites") ? "Favorites" : "Library";
        page(title, true); filterReload = () -> browse(0); retryAction = () -> browse(start); int current = generation;
        LinearLayout searchRow = new LinearLayout(this); EditText search = new EditText(this); search.setSingleLine(); search.setTextColor(Color.WHITE); search.setHintTextColor(Color.LTGRAY);
        search.setHint("Search " + title); search.setText(searchTerm); searchRow.addView(search, new LinearLayout.LayoutParams(0, -2, 1));
        addButton(searchRow, "Search", () -> {
            searchTerm = search.getText().toString().trim(); ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(search.getWindowToken(), 0); browse(0);
        });
        body.addView(searchRow); LinearLayout list = scrollBody(); loading("Loading titles…");
        JSONObject query = with(with(with(value("start", start), "search", searchTerm), "parent", parentId), "kind", category);
        with(query, "three_d_only",threeDOnly); with(query, "sort", prefs.getString("sort", "name")); if (category.equals("favorites")) with(query, "favorite", true);
        request("browse", query, data -> {
            JSONArray items = data.getJSONArray("items"); ready(items.length() == 0 ? (threeDOnly ? "No 3D titles found. Check the 3D format in Jellyfin or turn off 3D only." : "No matching titles. Try another search or library.") : "Select a title to see details and playback options");
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
        with(preparation,"hardware_hevc_8k60",VideoDecoderCapabilities.supportsHevc8k60());
        android.util.Log.i("Jellyfin3D", "Regular hardware HEVC 8K60=" + VideoDecoderCapabilities.supportsHevc8k60());
        with(preparation,"projection",prefs.getString("projection."+item,"Auto"));
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
        boolean supported = !format.equals("Mvc") && !format.equals("MvHevc") && !format.equals("Unspecified3d") && data.optInt("width") > 0 && data.optInt("height") > 0;
        content.addView(heading("Viewing mode"));
        RadioGroup views = new RadioGroup(this); views.setOrientation(LinearLayout.HORIZONTAL);
        String[] viewNames={"Cinema screen","VR180°","VR360°","Fisheye"}, viewValues={"Flat","Equirectangular180","Equirectangular360","Fisheye"};
        for(int i=0;i<viewNames.length;i++) {
            final String mode=viewValues[i]; RadioButton option=new RadioButton(this); option.setText(viewNames[i]); option.setTextColor(Color.WHITE); option.setTextSize(18); option.setMinHeight(dp(48)); option.setChecked(projection.equals(mode));
            option.setOnClickListener(v -> { with(data,"projection",mode); prefs.edit().putString("projection."+item,mode).apply(); }); views.addView(option);
        }
        content.addView(views);
        content.addView(label("Choose the projection that matches the source. Fisheye is for filename/tag-classified fisheye items. In VR, View opens tilt, 180°/360° and recenter controls.",17));
        content.addView(heading("Where would you like to watch?"));
        if (supported) addButton(content, "Watch in immersive cinema", () -> launchCinema(item, data));
        addButton(content, "Watch in Quest panel", () -> { try { playback(item, data); } catch (Exception e) { error("Could not start playback."); } });
        content.addView(label("Cinema: trigger pauses / resumes · thumbstick left / right seeks 10 seconds · grip changes audio · thumbstick up recenters the screen.", 18));
        if (!data.optString("method").equals("DirectPlay")) content.addView(label("Jellyfin selected server conversion. Resolution, bitrate or HDR may change.", 17));
        if (!supported) content.addView(label(format.equals("Mvc") ? "MVC needs a prepared full-SBS version on the server. Both eye views must be decoded; ordinary server transcoding only preserves the base view." : "This source's projection or stereo packing is not yet supported in immersive cinema.", 18));
        addButton(content, "Playback settings", this::settings); addButton(content, "Back to title", () -> details(item));
        String preferred = prefs.getString("player", "ask");
        if (preferred.equals("cinema") && supported) launchCinema(item, data);
        else if (preferred.equals("panel") && format.equals("Mono") && projection.equals("Flat")) { try { playback(item, data); } catch (Exception e) { error("Could not start playback."); } }
    }
    private void launchCinema(String item, JSONObject data) {
        Intent intent = new Intent(this, VrActivity.class); intent.addCategory("com.oculus.intent.category.VR"); intent.addCategory("org.khronos.openxr.intent.category.IMMERSIVE_HMD");
        intent.putExtra("server", server).putExtra("device", device).putExtra("item", item).putExtra("playback", data.toString())
            .putExtra("environment", prefs.getString("environment", prefs.getBoolean("passthrough", false) ? "passthrough" : "cinema"))
            .putExtra("passthrough", prefs.getString("environment", prefs.getBoolean("passthrough", false) ? "passthrough" : "cinema").equals("passthrough"))
            .putExtra("screen_width", QuestPreferences.screenWidth(prefs.getFloat("screen_width", QuestPreferences.DEFAULT_WIDTH)))
            .putExtra("screen_distance", QuestPreferences.screenDistance(prefs.getFloat("screen_distance", QuestPreferences.DEFAULT_DISTANCE)));
        loading("Starting immersive cinema…");
        int current = generation;
        try {
            CinemaLaunch.open(this, intent, new ResultReceiver(ui) {
                @Override protected void onReceiveResult(int code, Bundle result) {
                    if (isFinishing() || isDestroyed() || generation != current) return;
                    String failure = result == null ? null : result.getString("error");
                    if (failure == null) ready("Playback closed."); else error(failure);
                }
            });
        } catch (RuntimeException e) { error("Could not open the cinema window. Try again from the library."); }
    }
    private JSONObject playbackData(long ticks) {
        return with(with(with(with(value("item", activeItem), "source", activeSource), "session", activeSession == null ? JSONObject.NULL : activeSession), "ticks", ticks), "method", activeMethod);
    }
    private void playback(String item, JSONObject data) throws Exception {
        activeItem = item; activeMethod = data.optString("method", "DirectPlay"); activeSource = data.getString("source"); activeSession = data.isNull("session") ? null : data.getString("session");
        long resumeMs = data.optLong("resume") / 10000L; page(data.optString("title", "Now playing"), false); int current = generation;
        backAction = () -> { stopPlayback(); details(item); }; retryAction = () -> { stopPlayback(); try { playback(item, data); } catch (Exception e) { error("Could not reopen video."); } };
        playbackPage = true; chromePolicy.interact(SystemClock.uptimeMillis());
        // Chrome overlays the video, so hiding it never resizes the decoder surface.
        root.removeView(chrome); root.removeView(body); root.setPadding(0,0,0,0);
        FrameLayout stage = new FrameLayout(this); root.addView(stage,new LinearLayout.LayoutParams(-1,-1));
        stage.addView(body,new FrameLayout.LayoutParams(-1,-1));
        chrome.setPadding(dp(24),dp(14),dp(24),dp(10));
        chrome.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM,new int[]{0xF8101118,0xE8101118,0x90101118}));
        stage.addView(chrome,new FrameLayout.LayoutParams(-1,-2,Gravity.TOP));
        LinearLayout actions = new LinearLayout(this); addButton(actions, "Back to title", backAction); addButton(actions, "Audio", () -> tracks(MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO)); addButton(actions, "Subtitles", () -> tracks(MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_SUBTITLE)); chrome.addView(actions);
        FrameLayout frame = new FrameLayout(this); body.addView(frame, new LinearLayout.LayoutParams(-1, 0, 1));
        video = new VideoView(this); FrameLayout.LayoutParams videoLp = new FrameLayout.LayoutParams(-1, -1, Gravity.CENTER); frame.addView(video, videoLp);
        videoLoading = new FrameLayout(this); videoLoading.setBackgroundColor(0xB0101118);
        LinearLayout load = new LinearLayout(this); load.setOrientation(1); load.setGravity(Gravity.CENTER); load.addView(new ProgressBar(this)); load.addView(label("Opening video…", 22));
        videoLoading.addView(load, new FrameLayout.LayoutParams(-1, -1)); frame.addView(videoLoading, new FrameLayout.LayoutParams(-1, -1));
        panelControls = new MediaController(this); panelControls.setAnchorView(video); video.setMediaController(panelControls);
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
        video.setVideoURI(Uri.parse(data.getString("url")), headers); ui.post(chromeTick);
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
    private final Runnable chromeTick = new Runnable() {
        @Override public void run() {
            if (!playbackPage || video == null || isDestroyed()) return;
            boolean playing = started && video.isPlaying() && videoLoading.getVisibility() == View.GONE && hasWindowFocus();
            boolean visible = chromePolicy.visible(SystemClock.uptimeMillis(),playing,touchingPlayer || hoveringChrome);
            setChromeVisible(visible); ui.postDelayed(this,200);
        }
    };
    private void setChromeVisible(boolean visible) {
        if (!playbackPage) return;
        if (visible) {
            chrome.animate().cancel(); chrome.setVisibility(View.VISIBLE); chrome.setAlpha(1f);
        } else if (chrome.getVisibility() == View.VISIBLE && chrome.getAlpha() == 1f) {
            LinearLayout fading = chrome;
            fading.animate().alpha(0f).setDuration(180).withEndAction(() -> fading.setVisibility(View.GONE)).start();
        }
    }
    private void revealChrome() { if (playbackPage) { chromePolicy.interact(SystemClock.uptimeMillis()); setChromeVisible(true); } }
    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        if (playbackPage) {
            if (event.getActionMasked() == MotionEvent.ACTION_DOWN) {
                touchingPlayer = true; revealingChromeTouch = chrome.getVisibility() != View.VISIBLE || chrome.getAlpha() < 1f;
                revealChrome(); if (revealingChromeTouch && panelControls != null) panelControls.show();
            }
            else if (event.getActionMasked() == MotionEvent.ACTION_UP || event.getActionMasked() == MotionEvent.ACTION_CANCEL) { touchingPlayer = false; chromePolicy.interact(SystemClock.uptimeMillis()); }
        }
        if (revealingChromeTouch) {
            if (event.getActionMasked() == MotionEvent.ACTION_UP || event.getActionMasked() == MotionEvent.ACTION_CANCEL) revealingChromeTouch = false;
            return true;
        }
        return super.dispatchTouchEvent(event);
    }
    @Override public boolean dispatchGenericMotionEvent(MotionEvent event) {
        if (playbackPage) {
            hoveringChrome = event.getActionMasked() != MotionEvent.ACTION_HOVER_EXIT && chrome.getVisibility() == View.VISIBLE && event.getY() >= 0 && event.getY() <= chrome.getHeight();
            if (event.getActionMasked() == MotionEvent.ACTION_HOVER_ENTER || event.getActionMasked() == MotionEvent.ACTION_SCROLL) revealChrome();
        }
        return super.dispatchGenericMotionEvent(event);
    }
    @Override public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getAction() == KeyEvent.ACTION_DOWN) revealChrome();
        return super.dispatchKeyEvent(event);
    }
    private void progress(int current) {
        ui.postDelayed(() -> {
            if (current != generation || video == null || !started) return;
            lastPosition = video.getCurrentPosition(); request("progress", with(playbackData(lastPosition * 10000L), "paused", !video.isPlaying()), d -> {}, message -> {}); progress(current);
        }, 10000);
    }
    private void stopPlayback() {
        if (playbackPage) { chrome.animate().cancel(); chrome.setVisibility(View.VISIBLE); chrome.setAlpha(1f); }
        playbackPage = false; ui.removeCallbacks(chromeTick); if (panelControls != null) { panelControls.hide(); panelControls = null; }
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
        content.addView(heading("Cinema"));
        choice(content, "Environment", "environment", new String[]{"Dark cinema", "Room / passthrough", "Furnished loft"}, new String[]{"cinema", "passthrough", "loft"}, prefs.getBoolean("passthrough", false) ? "passthrough" : "cinema");
        toggle(content, "Resume from Jellyfin's saved position", "resume", true);
        slider(content, "Screen width", "screen_width", 1.5f, 6f, QuestPreferences.screenWidth(prefs.getFloat("screen_width", QuestPreferences.DEFAULT_WIDTH)));
        slider(content, "Viewing distance", "screen_distance", 1f, 5f, QuestPreferences.screenDistance(prefs.getFloat("screen_distance", QuestPreferences.DEFAULT_DISTANCE)));
        content.addView(label("Point and trigger / pinch to use the cinema controls. Grip or pinch an edge to move the screen; drag a corner to resize. Use Flatter / Curve + to bend it, or Recenter to bring it in front of you. Removing the headset pauses playback; press Play after returning.", 17));
        choice(content, "Library sorting", "sort", new String[]{"Title", "Recently added"}, new String[]{"name", "latest"}, "name");
        content.addView(heading("Poster size")); RadioGroup columns = new RadioGroup(this); int count = QuestPreferences.columns(prefs.getInt("columns", 5));
        for (int n = 3; n <= 6; n++) { final int value = n; RadioButton b = new RadioButton(this); b.setText(n + " columns"); b.setTextSize(18); b.setTextColor(Color.WHITE); b.setMinHeight(dp(48)); b.setChecked(n == count); b.setOnClickListener(v -> prefs.edit().putInt("columns", value).apply()); columns.addView(b); } content.addView(columns);
        content.addView(heading("Account & server")); content.addView(label((userName.isEmpty() ? "Jellyfin account" : userName) + "\n" + server, 18));
        addButton(content, "Change server", this::home);
        if (signedIn) addButton(content,"Switch user",this::users);
        if (signedIn) addButton(content, "Sign out", () -> { loading("Signing out…"); request("signout", null, d -> { signedIn = false; userName = ""; userId = ""; resetAccountView(); users(); }); });
        addButton(content, "Clear cached artwork", () -> { posterCache.evictAll(); ready("Artwork cache cleared."); });
        content.addView(heading("About")); content.addView(label("Jellyfin 3D for Quest · 0.1.0\nIndependent native Jellyfin client using DeskQuest XR components.\nStereo cinema supports mono, SBS and over-under. MVC, MV-HEVC and immersive subtitles are still unavailable.", 16));
        addButton(content, "Back", signedIn ? this::dashboard : this::home);
    }
    @Override public void onBackPressed() { if (backAction != null) backAction.run(); else super.onBackPressed(); }
    @Override protected void onStop() { super.onStop(); if (video != null) { String item = activeItem; stopPlayback(); details(item); } }
    @Override protected void onDestroy() { generation++; ui.removeCallbacks(chromeTick); io.shutdown(); reads.shutdownNow(); images.shutdownNow(); posterCache.evictAll(); super.onDestroy(); }
}
