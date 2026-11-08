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
    private TextView findButton(View view,String text) {
        if(view.getVisibility()!=View.VISIBLE || !view.isShown()) return null;
        if(view instanceof TextView && view.isClickable() && view.isEnabled() && ((TextView)view).getText().toString().equals(text)) return (TextView)view;
        if(view instanceof ViewGroup) for(int i=0;i<((ViewGroup)view).getChildCount();i++) { TextView found=findButton(((ViewGroup)view).getChildAt(i),text); if(found!=null)return found; }
        return null;
    }
    private void click(String text) {
        await(text); final boolean[] clicked={false};
        runOnMainSync(() -> { TextView button=findButton(activity.getWindow().getDecorView(),text); clicked[0]=button!=null && button.performClick(); });
        check(clicked[0],"Not clickable: " + text);
    }
    private android.widget.EditText searchInput(View view) {
        if (view instanceof android.widget.EditText) return (android.widget.EditText)view;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) { android.widget.EditText found = searchInput(((ViewGroup)view).getChildAt(i)); if (found != null) return found; }
        return null;
    }
    private int posters(View view) {
        int count = view instanceof ImageView && view.getContentDescription() != null && view.getContentDescription().toString().startsWith("Poster for ") && ((ImageView)view).getDrawable() instanceof BitmapDrawable ? 1 : 0;
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
    private Object field(Activity target,String name) {
        try { java.lang.reflect.Field field=target.getClass().getDeclaredField(name); field.setAccessible(true); return field.get(target); }
        catch(Exception e) { throw new RuntimeException(e); }
    }
    private boolean chromeVisible() {
        final boolean[] visible={false}; runOnMainSync(() -> { View chrome=(View)field(activity,"chrome"); visible[0]=chrome.getVisibility()==View.VISIBLE; }); return visible[0];
    }
    private int[] videoBounds() {
        final int[] size={0,0}; runOnMainSync(() -> { View video=(View)field(activity,"video"); size[0]=video.getWidth(); size[1]=video.getHeight(); }); return size;
    }
    private android.widget.EditText passwordInput(View view) {
        if (view instanceof android.widget.EditText && "Password".contentEquals(view.getContentDescription())) return (android.widget.EditText)view;
        if (view instanceof ViewGroup) for(int i=0;i<((ViewGroup)view).getChildCount();i++) { android.widget.EditText found=passwordInput(((ViewGroup)view).getChildAt(i)); if(found!=null)return found; }
        return null;
    }
    private boolean playing(Activity vr) {
        final boolean[] result = {false}; runOnMainSync(() -> { try { result[0] = (Boolean)vr.getClass().getMethod("isVideoPlaying").invoke(vr); } catch (Exception e) { throw new RuntimeException(e); } }); return result[0];
    }
    private String startupState(Activity vr, boolean requestPlay) {
        final String[] state = {""};
        runOnMainSync(() -> { try {
            java.lang.reflect.Field gateField = vr.getClass().getDeclaredField("playbackGate"); gateField.setAccessible(true);
            Object gate = gateField.get(vr);
            java.lang.reflect.Field focusedField = gate.getClass().getDeclaredField("focused"); focusedField.setAccessible(true);
            java.lang.reflect.Field wantsField = gate.getClass().getDeclaredField("wantsPlay"); wantsField.setAccessible(true);
            java.lang.reflect.Field readyField = vr.getClass().getDeclaredField("readyToPlay"); readyField.setAccessible(true);
            boolean focused = focusedField.getBoolean(gate), wants = wantsField.getBoolean(gate), ready = readyField.getBoolean(vr);
            // Simulate explicit Play only after an actual focus interruption. A prepared/buffering
            // player can want playback before isPlaying becomes true; toggling then pauses it.
            if (requestPlay && focused && ready && !wants) vr.getClass().getMethod("onXrCommand", int.class).invoke(vr, 0);
            state[0] = "focused=" + focused + " wantsPlay=" + wants + " ready=" + ready + " windowFocus=" + vr.hasWindowFocus();
        } catch (Exception e) { throw new RuntimeException(e); } });
        return state[0];
    }
    private void saveControls(Activity vr, String name) throws Exception {
        java.lang.reflect.Method art=vr.getClass().getMethod("controlsPixels",int.class,boolean.class,float.class);
        byte[] normal=(byte[])art.invoke(vr,-1,false,1f), hidden=(byte[])art.invoke(vr,-1,false,0f);
        check(normal.length==1536*784*4 && hidden.length==normal.length,"Invalid controls atlas");
        for(int i=0;i<1536*784;i++) check(hidden[4*i+3]==0,"Hidden controls remained opaque");
        java.lang.reflect.Method env=vr.getClass().getMethod("environmentPixels");
        byte[] room=(byte[])env.invoke(vr); check(room.length==512*256*4,"Invalid separate room atlas");
        byte[] roomAgain=(byte[])env.invoke(vr);check(java.util.Arrays.equals(room,roomAgain),"Controls changed room materials");
        saveAtlas(normal,name);
        saveAtlas((byte[])art.invoke(vr,5,true,1f),name+"-screen-tools");
    }
    private void saveAtlas(byte[] rgba,String name) throws Exception {
        int[] pixels=new int[1536*784];
        for(int i=0;i<pixels.length;i++) {
            int a=rgba[4*i+3]&255, r=0,g=0,b=0;
            if(a>0) { r=Math.min(255,(rgba[4*i]&255)*255/a); g=Math.min(255,(rgba[4*i+1]&255)*255/a); b=Math.min(255,(rgba[4*i+2]&255)*255/a); }
            pixels[i]=(a<<24)|(r<<16)|(g<<8)|b;
        }
        Bitmap bitmap=Bitmap.createBitmap(pixels,1536,784,Bitmap.Config.ARGB_8888);
        try(FileOutputStream out=new FileOutputStream(new File(activity.getExternalFilesDir(null),name+".png"))) { bitmap.compress(Bitmap.CompressFormat.PNG,100,out); } bitmap.recycle();
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
            click("3D only"); await("Stereo Feature");
            check(!contains("Test Feature"),"3D filter kept mono titles"); screenshot("3d-only");
            check(context.getSharedPreferences("app",0).getBoolean("3d_only",false),"3D filter not persisted");
            click("Home"); await("Stereo Feature"); check(!contains("Test Feature"),"Home did not preserve 3D filter");
            click("Movies"); await("Stereo Feature"); click("3D only"); await("Test Feature");
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
            runOnMainSync(() -> {
                long now=SystemClock.uptimeMillis();
                android.view.MotionEvent exit=android.view.MotionEvent.obtain(now,now,android.view.MotionEvent.ACTION_HOVER_EXIT,0,0,0);
                activity.dispatchGenericMotionEvent(exit);exit.recycle();
            });
            int[] before=videoBounds();
            long hideDeadline=SystemClock.uptimeMillis()+9000;
            while(chromeVisible() && SystemClock.uptimeMillis()<hideDeadline) SystemClock.sleep(100);
            check(!chromeVisible(),"Playback top bar did not auto-hide: playing="+((android.widget.VideoView)field(activity,"video")).isPlaying()+" focus="+activity.hasWindowFocus()+" hover="+field(activity,"hoveringChrome")+" touch="+field(activity,"touchingPlayer")); screenshot("panel-clean-video");
            check(java.util.Arrays.equals(before,videoBounds()),"Hiding chrome resized the video surface");
            runOnMainSync(() -> {
                long now=SystemClock.uptimeMillis(); View root=activity.getWindow().getDecorView();
                android.view.MotionEvent down=android.view.MotionEvent.obtain(now,now,0,root.getWidth()/2f,root.getHeight()/2f,0);
                android.view.MotionEvent up=android.view.MotionEvent.obtain(now,now+30,1,root.getWidth()/2f,root.getHeight()/2f,0);
                activity.dispatchTouchEvent(down); activity.dispatchTouchEvent(up); down.recycle(); up.recycle();
            });
            check(chromeVisible(),"Movie tap did not reveal top bar");
            runOnMainSync(() -> ((android.widget.VideoView)field(activity,"video")).pause()); SystemClock.sleep(600);
            check(chromeVisible(),"Paused video lost top bar");
            click("Back to title"); await("Play from beginning");
            click("Settings"); await("Default player"); screenshot("settings");
            click("Quest panel");
            check("panel".equals(context.getSharedPreferences("app", 0).getString("player", "")), "Player preference not saved");
            click("Home"); await("Continue Watching");
            click("Viewer ▾"); await("On this Quest"); click("Guest"); await("Sign in to this server");
            runOnMainSync(() -> passwordInput(activity.getWindow().getDecorView()).setText("wrong-password")); click("Sign in");
            await("Jellyfin did not authorize this account. Check your credentials or use Quick Connect.");
            runOnMainSync(() -> passwordInput(activity.getWindow().getDecorView()).setText("test-password")); click("Sign in");
            await("Welcome, Guest"); await("Guest Feature"); check(!contains("Test Feature"),"Old account titles survived switch"); screenshot("guest-home");
            click("Guest ▾"); await("On this Quest"); await("Guest · Current"); screenshot("user-picker"); click("Viewer");
            await("Welcome, Viewer"); await("Test Feature");
            click("Shows"); await("Sample Series");
            runOnMainSync(() -> { View view = find(activity.getWindow().getDecorView(), "Sample Series"); while (!view.isClickable()) view = (View)view.getParent(); view.performClick(); });
            await("S1 E1\nEpisode One"); screenshot("episodes");
            // Each background uses the same real surface decoder, transport atlas and stereo renderer.
            for (String environment : new String[]{"cinema", "passthrough", "loft", "vr180", "vr360"}) {
                final boolean spherical = environment.startsWith("vr");
                context.getSharedPreferences("app", 0).edit().putString("environment", spherical ? "cinema" : environment)
                    .putFloat("screen_curve", environment.equals("passthrough") ? 0f : 1.2f).commit();
                ActivityMonitor vrMonitor = addMonitor("org.questjellyfin.player.VrActivity", null, false);
                JSONObject videoData = new JSONObject().put("title", "XR smoke video").put("url", url + "/Videos/movie/stream")
                    .put("authorization", "MediaBrowser Token=\"smoke-token\"").put("projection", environment.equals("vr180") ? "Equirectangular180" : environment.equals("vr360") ? "Equirectangular360" : "Flat").put("format", (spherical || environment.equals("loft")) ? "HalfSideBySide" : "Mono").put("eye_order", "LeftRight")
                    .put("width", 640).put("height", 360).put("method", "DirectPlay").put("source", "source").put("session", JSONObject.NULL);
                runOnMainSync(() -> {
                    try {
                        java.lang.reflect.Method launchCinema = activity.getClass().getDeclaredMethod("launchCinema", String.class, JSONObject.class);
                        launchCinema.setAccessible(true); launchCinema.invoke(activity, "movie", videoData);
                    } catch (Exception e) { throw new RuntimeException(e); }
                });
                Activity vr = waitForMonitorWithTimeout(vrMonitor, 10000);
                check(vr != null, "Cinema did not open: " + environment);
                check(vr.getTaskId() != activity.getTaskId(), "Cinema inherited the resizable panel task");
                check(vr.getDisplay().getDisplayId() == android.view.Display.DEFAULT_DISPLAY, "Cinema inherited the panel display");
                java.lang.reflect.Method command = vr.getClass().getMethod("onXrCommand", int.class);
                java.lang.reflect.Method submitted = vr.getClass().getMethod("hasSubmittedVideoLayer");
                java.lang.reflect.Method controls = vr.getClass().getMethod("hasSubmittedControls");
                long frameDeadline = SystemClock.uptimeMillis() + 25000;
                while ((!((Boolean)submitted.invoke(vr)) || !playing(vr)) && SystemClock.uptimeMillis() < frameDeadline) {
                    startupState(vr, true);
                    SystemClock.sleep(200);
                }
                check((Boolean)submitted.invoke(vr), "No submitted video layer: " + environment);
                check((Boolean)controls.invoke(vr), "No submitted controls layer: " + environment);
                check((Boolean)vr.getClass().getMethod("hasSubmittedSphericalLayer").invoke(vr) == spherical, "Incorrect compositor layer type: " + environment);
                check(playing(vr), "Video did not start: " + environment + " " + startupState(vr, false));
                java.lang.reflect.Method count=vr.getClass().getMethod("currentVideoLayerCount");
                int expectedLayers=(spherical || environment.equals("loft")) ? 2 : 1;
                check((Integer)count.invoke(vr)==expectedLayers,"Incorrect eye layer count");
                runOnMainSync(() -> { try {
                    Object presentation=field(vr,"presentation");
                    java.lang.reflect.Method buffering=presentation.getClass().getDeclaredMethod("buffering",boolean.class);buffering.setAccessible(true);buffering.invoke(presentation,true);
                    java.lang.reflect.Method publish=vr.getClass().getDeclaredMethod("publishPresentation");publish.setAccessible(true);publish.invoke(vr);
                }catch(Exception e){throw new RuntimeException(e);} });
                SystemClock.sleep(500);check((Integer)count.invoke(vr)==0,"Stale stereo video remained visible during buffering");
                check((Boolean)controls.invoke(vr),"Buffering lost controls");
                runOnMainSync(() -> { try {
                    Object presentation=field(vr,"presentation");
                    java.lang.reflect.Method buffering=presentation.getClass().getDeclaredMethod("buffering",boolean.class);buffering.setAccessible(true);buffering.invoke(presentation,false);
                    java.lang.reflect.Method publish=vr.getClass().getDeclaredMethod("publishPresentation");publish.setAccessible(true);publish.invoke(vr);
                }catch(Exception e){throw new RuntimeException(e);} });
                SystemClock.sleep(500);check((Integer)count.invoke(vr)==expectedLayers,"Video did not return after buffering");
                command.invoke(vr, 0); SystemClock.sleep(200); check(!playing(vr), "Pause failed");
                command.invoke(vr, 0); SystemClock.sleep(200); check(playing(vr), "Play failed");
                java.lang.reflect.Field epochField = vr.getClass().getDeclaredField("xrEpoch"); epochField.setAccessible(true);
                long epoch = epochField.getLong(vr);
                java.lang.reflect.Method focus = vr.getClass().getMethod("onXrFocusChanged", long.class, boolean.class);
                focus.invoke(vr, epoch - 1, false); SystemClock.sleep(150); check(playing(vr), "Stale XR callback paused current playback");
                focus.invoke(vr, epoch, false); SystemClock.sleep(200); check(!playing(vr), "Focus loss did not pause MediaPlayer");
                focus.invoke(vr, epoch, true); SystemClock.sleep(200); check(!playing(vr), "Focus return auto-resumed playback");
                command.invoke(vr, 0); SystemClock.sleep(200); check(playing(vr), "Explicit resume failed");
                vr.getClass().getMethod("onXrSeek", int.class).invoke(vr, 500); SystemClock.sleep(500);
                final int[] position = {0};
                runOnMainSync(() -> { try {
                    java.lang.reflect.Field playerField = vr.getClass().getDeclaredField("player"); playerField.setAccessible(true);
                    position[0] = ((android.media.MediaPlayer)playerField.get(vr)).getCurrentPosition();
                } catch (Exception e) { throw new RuntimeException(e); } });
                check(position[0] >= 13000 && position[0] <= 18000, "Timeline seek failed: " + position[0]);
                command.invoke(vr, 3); SystemClock.sleep(200); // Empty audio list is a visible, recoverable state.
                saveControls(vr, "controls-" + environment);
                command.invoke(vr, 4); await("Playback closed."); removeMonitor(vrMonitor);
            }
            result.putString("stream", "OK: service logo/artwork, playback chrome idle-hide/reveal/pause and stable surface, persisted server-side 3D filter, credential failure/retry and remembered user switching, home, movies, details, favorites, loading-to-panel-playback, settings persistence, series episodes, request errors and retry, separate VR task, cinema/passthrough/loft control and video layers, stereo cylinder, per-eye VR180/VR360 equirect layers, play/pause/seek/audio/exit, stale callback rejection and focus-loss pause\n"); finish(-1, result);
        } catch (Throwable error) { result.putString("stream", "FAILED: " + error + "\n"); finish(0, result); }
    }
}
