package org.questjellyfin.player;

import android.app.Activity;
import android.app.ActivityOptions;
import android.content.Intent;
import android.os.ResultReceiver;
import android.view.Display;

/** A cinema owns its task on the headset display, independently of the resizable panel. */
final class CinemaLaunch {
    private CinemaLaunch() {}
    static void open(Activity caller, Intent intent, ResultReceiver result) {
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        intent.putExtra("cinema_result", result);
        ActivityOptions options = ActivityOptions.makeBasic();
        options.setLaunchDisplayId(Display.DEFAULT_DISPLAY);
        caller.startActivity(intent, options.toBundle());
    }
}
