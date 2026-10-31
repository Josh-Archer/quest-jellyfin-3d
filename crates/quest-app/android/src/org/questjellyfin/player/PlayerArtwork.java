package org.questjellyfin.player;

import android.graphics.*;
import android.text.TextPaint;
import android.text.TextUtils;

/** Movie-attached transport artwork. Pixel targets are shared with screen_controls.rs. */
final class PlayerArtwork {
    private static final int WHITE = Color.rgb(245, 247, 252);
    private static final int ACCENT = Color.rgb(0, 164, 220);
    static byte[] pixels(String title, String status, String time, String audio, String screen,
                         boolean playing, boolean loading, float progressValue, int hovered, boolean tools, float opacity) {
        Bitmap image = Bitmap.createBitmap(512, 256, Bitmap.Config.ARGB_8888);
        Canvas c = new Canvas(image);
        Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        int height = tools ? 160 : 96;
        p.setShader(new LinearGradient(0, 0, 0, height, 0x50202732, 0xF0080C14, Shader.TileMode.CLAMP));
        c.drawRect(0, 0, 512, height, p); p.setShader(null);
        c.save(); c.translate(0, tools ? 64 : 0);
        p.setColor(0x24FFFFFF); c.drawRect(0, 0, 512, 1, p);
        p.setTypeface(Typeface.DEFAULT_BOLD); p.setTextSize(14); p.setColor(WHITE);
        c.drawText(TextUtils.ellipsize(title, new TextPaint(p), 326, TextUtils.TruncateAt.END).toString(), 16, 18, p);
        p.setTypeface(Typeface.DEFAULT); p.setTextSize(11); p.setColor(0xCCDFE7F1);
        p.setTextAlign(Paint.Align.RIGHT); c.drawText(status, 456, 17, p); p.setTextAlign(Paint.Align.LEFT);
        highlight(c, p, hovered == 4, 470, 2, 506, 23);
        p.setColor(0xDDEAF0F8); p.setStrokeWidth(1.7f); c.drawLine(484, 8, 492, 16, p); c.drawLine(492, 8, 484, 16, p);
        float progress = Math.max(0, Math.min(1, progressValue));
        p.setColor(0x55FFFFFF); c.drawRoundRect(16, 30, 496, 34, 2, 2, p);
        p.setColor(ACCENT); c.drawRoundRect(16, 30, 16 + 480 * progress, 34, 2, 2, p);
        p.setColor(WHITE); c.drawCircle(16 + 480 * progress, 32, hovered == 12 ? 5 : 3, p);
        p.setColor(hovered == 0 ? Color.rgb(48, 192, 240) : ACCENT); c.drawCircle(40, 69, 20, p);
        p.setColor(WHITE);
        if (playing) { c.drawRoundRect(33, 61, 38, 77, 1, 1, p); c.drawRoundRect(42, 61, 47, 77, 1, 1, p); }
        else { Path play = new Path(); play.moveTo(35, 59); play.lineTo(49, 69); play.lineTo(35, 79); play.close(); c.drawPath(play, p); }
        seek(c, p, 96, false, hovered == 1); seek(c, p, 146, true, hovered == 2);
        p.setTextAlign(Paint.Align.LEFT); p.setTypeface(Typeface.MONOSPACE); p.setTextSize(11); p.setColor(0xFFDDE5F0);
        c.drawText(time, 181, 73, p); p.setTypeface(Typeface.DEFAULT);
        pill(c,p,286,358,audio,hovered==3,false);
        pill(c,p,364,448,"Screen",hovered==5,tools);
        highlight(c,p,hovered==6,464,49,504,89);
        p.setColor(0xFFDDE5F0); p.setStrokeWidth(2); c.drawLine(477,65,484,72,p); c.drawLine(484,72,491,65,p);
        if (loading) {
            p.setStyle(Paint.Style.STROKE); p.setStrokeWidth(2); p.setColor(ACCENT);
            c.drawArc(348,7,360,19,(android.os.SystemClock.uptimeMillis()/5)%360,260,false,p); p.setStyle(Paint.Style.FILL);
        }
        c.restore();
        if (tools) {
            p.setColor(0x28FFFFFF); c.drawRect(16,63,496,64,p);
            p.setColor(0xBBDCE5F2); p.setTextSize(11); p.setTextAlign(Paint.Align.LEFT);
            c.drawText("SCREEN  ·  " + screen + "  ·  Grip / pinch edges; drag corners",16,16,p);
            String[] names = {"Smaller","Larger","Flatter","Curve +","Recenter"};
            for (int i=0;i<5;i++) {
                float left=i*102.4f+6;
                p.setColor(hovered==i+7 ? 0x663FAFD5 : 0x22FFFFFF); c.drawRoundRect(left,24,left+90.4f,60,7,7,p);
                p.setColor(WHITE); p.setTextSize(13); p.setTextAlign(Paint.Align.CENTER); c.drawText(names[i],left+45.2f,47,p);
            }
        }
        drawLoft(c,p);
        int[] argb = new int[512*256]; image.getPixels(argb,0,512,0,0,512,256); image.recycle();
        byte[] rgba = new byte[argb.length*4];
        float alpha = Float.isFinite(opacity) ? Math.max(0,Math.min(1,opacity)) : 1;
        for (int i=0;i<argb.length;i++) {
            int value=argb[i], a=value>>>24;
            if (i/512 < 192) a=Math.round(a*alpha);
            // OpenXR's default texture-alpha blend expects premultiplied RGB.
            rgba[4*i]=(byte)(((value>>16&255)*a+127)/255); rgba[4*i+1]=(byte)(((value>>8&255)*a+127)/255);
            rgba[4*i+2]=(byte)(((value&255)*a+127)/255); rgba[4*i+3]=(byte)a;
        }
        return rgba;
    }
    private static void highlight(Canvas c, Paint p, boolean active, float left,float top,float right,float bottom) {
        if (active) { p.setColor(0x33FFFFFF); c.drawRoundRect(left,top,right,bottom,8,8,p); }
    }
    private static void pill(Canvas c,Paint p,float left,float right,String label,boolean hovered,boolean selected) {
        p.setColor(selected ? 0x773292B5 : hovered ? 0x44FFFFFF : 0x14FFFFFF); c.drawRoundRect(left,49,right,89,8,8,p);
        p.setTextAlign(Paint.Align.CENTER); p.setTextSize(12); p.setColor(WHITE);
        c.drawText(TextUtils.ellipsize(label,new TextPaint(p),right-left-12,TextUtils.TruncateAt.END).toString(),(left+right)/2,73,p);
    }
    private static void seek(Canvas c,Paint p,float x,boolean forward,boolean hovered) {
        highlight(c,p,hovered,x-22,47,x+22,91);
        p.setColor(0xFFE9EFF7); p.setStyle(Paint.Style.STROKE); p.setStrokeWidth(1.8f);
        c.drawArc(x-12,56,x+12,80,forward ? 205 : -25,300,false,p); p.setStyle(Paint.Style.FILL);
        Path arrow=new Path(); float sign=forward?1:-1;
        arrow.moveTo(x+sign*13,56); arrow.lineTo(x+sign*13,63); arrow.lineTo(x+sign*6,59); arrow.close(); c.drawPath(arrow,p);
        p.setTextAlign(Paint.Align.CENTER); p.setTextSize(10); p.setTypeface(Typeface.DEFAULT_BOLD); c.drawText("10",x,72,p); p.setTypeface(Typeface.DEFAULT);
    }
    private static void drawLoft(Canvas canvas, Paint p) {
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
    }
}
