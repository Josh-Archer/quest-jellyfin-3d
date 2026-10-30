package org.questjellyfin.tests;

import org.json.JSONObject;
import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.drawable.BitmapDrawable;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import android.widget.ImageView;
import java.io.File;
import java.io.FileOutputStream;

/** Exercises an isolated test copy; the installed user's server and session are untouched. */
public class DeviceSmoke extends Instrumentation {
    private Bundle arguments;
    private Activity activity;
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); this.arguments = arguments; start(); }
    private void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
    private TextView find(View view, String text) {
        if (view.getVisibility() != View.VISIBLE || !view.isShown()) return null;
        if (view instanceof TextView && ((TextView)view).getText().toString().equals(text)) return (TextView)view;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) { TextView found = find(((ViewGroup)view).getChildAt(i), text); if (found != null) return found; }
        return null;
    }
    private boolean contains(String text) {
        final boolean[] present = {false}; runOnMainSync(() -> present[0] = find(activity.getWindow().getDecorView(), text) != null); return present[0];
    }
    private void await(String text) {
        long end = SystemClock.uptimeMillis() + 25000;
        while (!contains(text) && SystemClock.uptimeMillis() < end) SystemClock.sleep(100);
        check(contains(text), "Missing UI: " + text);
    }
    private void click(String text) { await(text); runOnMainSync(() -> { TextView view = find(activity.getWindow().getDecorView(), text); check(view.performClick(), "Not clickable: " + text); }); }
    private android.widget.EditText searchInput(View view) {
        if (view instanceof android.widget.EditText) return (android.widget.EditText)view;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) { android.widget.EditText found = searchInput(((ViewGroup)view).getChildAt(i)); if (found != null) return found; }
        return null;
    }
    private int posters(View view) {
        int count = view instanceof ImageView && ((ImageView)view).getDrawable() instanceof BitmapDrawable ? 1 : 0;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) count += posters(((ViewGroup)view).getChildAt(i));
        return count;
    }
    private void screenshot(String name) {
        runOnMainSync(() -> {
            try {
                View root = activity.getWindow().getDecorView(); Bitmap bitmap = Bitmap.createBitmap(root.getWidth(), root.getHeight(), Bitmap.Config.ARGB_8888); root.draw(new Canvas(bitmap));
                File file = new File(activity.getExternalFilesDir(null), name + ".png"); try (FileOutputStream out = new FileOutputStream(file)) { bitmap.compress(Bitmap.CompressFormat.PNG, 100, out); } bitmap.recycle();
            } catch (Exception e) { throw new RuntimeException(e); }
        });
    }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            String url = arguments.getString("server");
            android.content.Context context = getTargetContext();
            context.getSharedPreferences("app", 0).edit().putString("server", url).putString("device", "smoke-device").commit();
            String session = "{\"server_url\":\"" + url + "\",\"access_token\":\"smoke-token\",\"user_id\":\"user\",\"user_name\":\"Viewer\",\"server_id\":\"mock\",\"device_id\":\"smoke-device\"}";
            try (FileOutputStream out = context.openFileOutput("jellyfin-session.json", 0)) { out.write(session.getBytes("UTF-8")); }
            Intent launch = new Intent().setClassName(context.getPackageName(), "org.questjellyfin.player.MainActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity = startActivitySync(launch); await("Recently Added");
            long end = SystemClock.uptimeMillis() + 15000; final int[] art = {0};
            do { runOnMainSync(() -> art[0] = posters(activity.getWindow().getDecorView())); if (art[0] > 0) break; SystemClock.sleep(100); } while (SystemClock.uptimeMillis() < end);
            check(art[0] > 0, "Poster bytes never reached ImageViews"); screenshot("home");
            click("Movies"); await("Select a title to see details and playback options"); screenshot("movies");
            runOnMainSync(() -> {
                android.widget.EditText search = searchInput(activity.getWindow().getDecorView());
                search.setText("fail-once"); find((View)search.getParent(), "Search").performClick();
            });
            await("Retry"); screenshot("request-error"); click("Retry"); await("Select a title to see details and playback options");
            // Poster cards are clickable containers; clicking their title bubbles through the container explicitly.
            runOnMainSync(() -> {
                TextView title = find(activity.getWindow().getDecorView(), "Test Feature"); check(title != null, "Movie missing");
                View view = title; while (!view.isClickable()) view = (View)view.getParent(); check(view.performClick(), "Movie card click failed");
            });
            await("Resume from 0:02"); await("A fixture feature for device testing."); screenshot("details");
            click("♡ Add favorite"); await("♥ Remove favorite");
            click("Play from beginning"); await("Requesting playback from Jellyfin…"); screenshot("opening-video"); await("Where would you like to watch?"); screenshot("playback-choice");
            click("Watch in Quest panel"); await("Playing"); screenshot("panel-playing");
            click("Back to title"); await("Play from beginning");
            click("Settings"); await("Default player"); screenshot("settings");
            click("Quest panel");
            check("panel".equals(context.getSharedPreferences("app", 0).getString("player", "")), "Player preference not saved");
            click("Home"); await("Continue Watching");
            click("Shows"); await("Sample Series");
            runOnMainSync(() -> { View view = find(activity.getWindow().getDecorView(), "Sample Series"); while (!view.isClickable()) view = (View)view.getParent(); view.performClick(); });
            await("S1 E1\nEpisode One"); screenshot("episodes");
            // Exercise the actual panel-to-cinema launch rather than starting VR directly.
            ActivityMonitor vrMonitor = addMonitor("org.questjellyfin.player.VrActivity", null, false);
            JSONObject videoData = new JSONObject().put("title", "XR smoke video").put("url", url + "/Videos/movie/stream")
                .put("authorization", "MediaBrowser Token=\"smoke-token\"").put("format", "Mono").put("eye_order", "LeftRight")
                .put("width", 640).put("height", 360).put("method", "DirectPlay").put("source", "source").put("session", JSONObject.NULL);
            runOnMainSync(() -> {
                try {
                    java.lang.reflect.Method launchCinema = activity.getClass().getDeclaredMethod("launchCinema", String.class, JSONObject.class);
                    launchCinema.setAccessible(true); launchCinema.invoke(activity, "movie", videoData);
                } catch (Exception e) { throw new RuntimeException(e); }
            });
            Activity vr = waitForMonitorWithTimeout(vrMonitor, 10000);
            check(vr != null, "Cinema did not open");
            check(vr.getTaskId() != activity.getTaskId(), "Cinema inherited the resizable panel task");
            check(vr.getDisplay().getDisplayId() == android.view.Display.DEFAULT_DISPLAY, "Cinema inherited the panel display");
            long frameDeadline = SystemClock.uptimeMillis() + 12000;
            java.lang.reflect.Method submitted = vr.getClass().getMethod("hasSubmittedVideoLayer");
            while (!((Boolean)submitted.invoke(vr)) && SystemClock.uptimeMillis() < frameDeadline) SystemClock.sleep(100);
            check((Boolean)submitted.invoke(vr), "Decoder output never reached a submitted cinema layer");
            SystemClock.sleep(2500);
            runOnMainSync(vr::finish); await("Playback closed.");
            result.putString("stream", "OK: artwork, home, movies, details, favorites, loading-to-panel-playback, settings persistence, series episodes, request errors and retry, separate VR task and submitted video layer\n"); finish(-1, result);
        } catch (Throwable error) { result.putString("stream", "FAILED: " + error + "\n"); finish(0, result); }
    }
}
