package org.questjellyfin.player;

import android.graphics.*;
import android.text.TextPaint;
import android.text.TextUtils;

/** High-resolution movie-attached Horizon-style transport. Layout matches screen_controls.rs. */
final class PlayerArtwork {
    static final int SUBTITLE_WIDTH = 1024, SUBTITLE_HEIGHT = 256;
    static byte[] pixels(String title, String status, String time, String audio, String screen,
                         boolean playing, boolean spherical, boolean loading, float progressValue,
                         int hovered, int pressed, boolean tools, float opacity, boolean mono, boolean swapped) {
        return pixels(title,status,time,audio,"Subs",screen,playing,spherical,loading,progressValue,hovered,pressed,tools,opacity,mono,swapped,0f,false);
    }
    static byte[] pixels(String title, String status, String time, String audio, String screen,
                         boolean playing, boolean spherical, boolean loading, float progressValue,
                         int hovered, int pressed, boolean tools, float opacity, boolean mono, boolean swapped,
                         float alignment, boolean alignmentAvailable) {
        return pixels(title,status,time,audio,"Subs",screen,playing,spherical,loading,progressValue,hovered,pressed,tools,opacity,mono,swapped,alignment,alignmentAvailable);
    }
    static byte[] pixels(String title, String status, String time, String audio, String subs, String screen,
                         boolean playing, boolean spherical, boolean loading, float progressValue,
                         int hovered, int pressed, boolean tools, float opacity, boolean mono, boolean swapped,
                         float alignment, boolean alignmentAvailable) {
        Bitmap bitmap = Bitmap.createBitmap(HorizonStyle.WIDTH * HorizonStyle.SCALE,
            HorizonStyle.TOOLS_HEIGHT * HorizonStyle.SCALE, Bitmap.Config.ARGB_8888);
        Canvas c = new Canvas(bitmap); c.scale(HorizonStyle.SCALE, HorizonStyle.SCALE);
        Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        int height = tools ? HorizonStyle.TOOLS_HEIGHT : HorizonStyle.MAIN_HEIGHT;
        p.setColor(HorizonStyle.SURFACE); c.drawRoundRect(0, 0, 768, height, 24, 24, p);
        p.setStyle(Paint.Style.STROKE); p.setStrokeWidth(1); p.setColor(0x426F7783);
        c.drawRoundRect(.5f, .5f, 767.5f, height-.5f, 24, 24, p); p.setStyle(Paint.Style.FILL);
        c.save(); c.translate(0, tools ? HorizonStyle.TOOLS_OFFSET : 0);
        text(c,p,title,24,36,22,HorizonStyle.TEXT,true,510);
        text(c,p,screen + (mono ? " · 2D view" : " · Stereo"),24,61,16,HorizonStyle.TEXT_SECONDARY,false,590);
        button(c,p,hovered,pressed,4,696,8,60,60,false,false);
        icon(c,p,"close",726,38,HorizonStyle.TEXT);
        float value=Math.max(0,Math.min(1,progressValue));
        p.setColor(HorizonStyle.OUTLINE); c.drawRoundRect(24,84,744,90,3,3,p);
        p.setColor(HorizonStyle.ACCENT); c.drawRoundRect(24,84,24+720*value,90,3,3,p);
        p.setColor(HorizonStyle.TEXT); c.drawCircle(24+720*value,87,hovered==12 ? 9 : 6,p);
        button(c,p,hovered,pressed,0,24,120,64,64,true,false);
        icon(c,p,playing ? "pause" : "play",56,152,HorizonStyle.ON_PRIMARY);
        button(c,p,hovered,pressed,1,100,122,60,60,false,false); seek(c,p,130,152,false);
        button(c,p,hovered,pressed,2,168,122,60,60,false,false); seek(c,p,198,152,true);
        text(c,p,time,236,158,18,HorizonStyle.TEXT,false,118);
        button(c,p,hovered,pressed,3,364,122,96,60,false,false);
        centered(c,p,audio,412,158,18,HorizonStyle.TEXT,84);
        button(c,p,hovered,pressed,18,472,122,96,60,false,false);
        centered(c,p,subs,520,158,18,HorizonStyle.TEXT,84);
        button(c,p,hovered,pressed,5,580,122,96,60,false,tools);
        centered(c,p,spherical ? "View" : "Screen",628,158,18,HorizonStyle.TEXT,84);
        button(c,p,hovered,pressed,6,696,122,60,60,false,false);
        icon(c,p,"down",726,152,HorizonStyle.TEXT);
        if (loading) {
            p.setStyle(Paint.Style.STROKE); p.setStrokeWidth(3); p.setColor(HorizonStyle.ACCENT);
            c.drawArc(552,26,576,50,(android.os.SystemClock.uptimeMillis()/6)%360,250,false,p); p.setStyle(Paint.Style.FILL);
        }
        text(c,p,status,loading ? 586 : 554,44,16,HorizonStyle.TEXT_SECONDARY,false,loading ? 102 : 126);
        c.restore();
        if (tools) {
            text(c,p,spherical ? (alignmentAvailable ? String.format(java.util.Locale.ROOT,"View settings · Alignment %+.1f°",alignment) : "View settings") : "Screen settings",24,30,20,HorizonStyle.TEXT,true,680);
            String[] names=spherical ? new String[]{"Tilt −","Tilt +","180°","360°","Recenter"} : new String[]{"Smaller","Larger","Flatter","Curve +","Recenter"};
            for(int i=0;i<5;i++) {
                float left=24+i*146.4f;
                button(c,p,hovered,pressed,7+i,left,42,134.4f,60,false,spherical && ((i==2 && screen.startsWith("VR180")) || (i==3 && screen.startsWith("VR360"))));
                centered(c,p,names[i],left+67.2f,79,18,HorizonStyle.TEXT,116);
            }
            button(c,p,hovered,pressed,14,24,116,200,60,false,mono);
            centered(c,p,mono ? "2D view" : "Stereo view",124,153,18,HorizonStyle.TEXT,176);
            button(c,p,hovered,pressed,13,240,116,200,60,false,swapped);
            centered(c,p,swapped ? "Eyes swapped" : "Swap eyes",340,153,18,HorizonStyle.TEXT,176);
            if (alignmentAvailable) {
                button(c,p,hovered,pressed,15,464,116,80,60,false,false);
                centered(c,p,"Align −",504,153,18,HorizonStyle.TEXT,68);
                button(c,p,hovered,pressed,16,556,116,80,60,false,false);
                centered(c,p,"Align +",596,153,18,HorizonStyle.TEXT,68);
                button(c,p,hovered,pressed,17,648,116,96,60,false,false);
                centered(c,p,"Reset",696,153,18,HorizonStyle.TEXT,80);
            } else {
                text(c,p,"Choose the view that",464,139,16,HorizonStyle.TEXT_SECONDARY,false,280);
                text(c,p,"feels comfortable.",464,162,16,HorizonStyle.TEXT_SECONDARY,false,280);
            }
            p.setColor(0x44707882); c.drawRect(24,191,744,192,p);
        }
        return rgba(bitmap,opacity);
    }
    /** Screen-attached cue with a translucent scrim. Empty text yields a fully transparent atlas. */
    static byte[] subtitlePixels(String cue) {
        Bitmap bitmap = Bitmap.createBitmap(SUBTITLE_WIDTH, SUBTITLE_HEIGHT, Bitmap.Config.ARGB_8888);
        if (cue == null || cue.trim().isEmpty()) return rgba(bitmap, 1f);
        Canvas c = new Canvas(bitmap);
        Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        String[] lines = cue.trim().replace('\r', '\n').split("\n");
        if (lines.length > 3) {
            lines = new String[]{lines[0], lines[1], lines[2]};
        }
        float size = lines.length > 2 ? 36f : 44f;
        TextPaint tp = new TextPaint(p);
        tp.setTypeface(HorizonStyle.medium);
        tp.setTextSize(size);
        tp.setColor(Color.WHITE);
        tp.setTextAlign(Paint.Align.CENTER);
        float lineHeight = size * 1.25f;
        float block = lines.length * lineHeight;
        float top = (SUBTITLE_HEIGHT - block) * 0.5f;
        float maxWidth = 0f;
        String[] drawn = new String[lines.length];
        for (int i = 0; i < lines.length; i++) {
            drawn[i] = TextUtils.ellipsize(lines[i], tp, SUBTITLE_WIDTH - 80f, TextUtils.TruncateAt.END).toString();
            maxWidth = Math.max(maxWidth, tp.measureText(drawn[i]));
        }
        float scrimWidth = Math.min(SUBTITLE_WIDTH - 24f, maxWidth + 48f);
        float scrimLeft = (SUBTITLE_WIDTH - scrimWidth) * 0.5f;
        float scrimTop = Math.max(8f, top - 16f);
        float scrimBottom = Math.min(SUBTITLE_HEIGHT - 8f, top + block + 16f);
        p.setColor(0xB0101118);
        c.drawRoundRect(scrimLeft, scrimTop, scrimLeft + scrimWidth, scrimBottom, 18, 18, p);
        float y = top + size;
        for (String line : drawn) {
            c.drawText(line, SUBTITLE_WIDTH * 0.5f, y, tp);
            y += lineHeight;
        }
        return rgba(bitmap, 1f);
    }
    static byte[] environmentPixels() {
        Bitmap bitmap=Bitmap.createBitmap(512,256,Bitmap.Config.ARGB_8888);
        drawLoft(new Canvas(bitmap),new Paint(Paint.ANTI_ALIAS_FLAG)); return rgba(bitmap,1);
    }
    private static byte[] rgba(Bitmap bitmap,float opacity) {
        int[] pixels=new int[bitmap.getWidth()*bitmap.getHeight()]; bitmap.getPixels(pixels,0,bitmap.getWidth(),0,0,bitmap.getWidth(),bitmap.getHeight()); bitmap.recycle();
        byte[] rgba=new byte[pixels.length*4]; float alpha=Float.isFinite(opacity) ? Math.max(0,Math.min(1,opacity)) : 1;
        for(int i=0;i<pixels.length;i++) {
            int pixel=pixels[i],a=Math.round((pixel>>>24)*alpha);
            rgba[4*i]=(byte)(((pixel>>16&255)*a+127)/255); rgba[4*i+1]=(byte)(((pixel>>8&255)*a+127)/255);
            rgba[4*i+2]=(byte)(((pixel&255)*a+127)/255); rgba[4*i+3]=(byte)a;
        }
        return rgba;
    }
    private static void text(Canvas c,Paint p,String value,float x,float y,float size,int color,boolean strong,float width) {
        p.setTextAlign(Paint.Align.LEFT);p.setTypeface(strong ? HorizonStyle.medium : HorizonStyle.regular);p.setTextSize(size);p.setColor(color);
        c.drawText(TextUtils.ellipsize(value,new TextPaint(p),width,TextUtils.TruncateAt.END).toString(),x,y,p);
    }
    private static void centered(Canvas c,Paint p,String value,float x,float y,float size,int color,float width) {
        p.setTypeface(HorizonStyle.medium);p.setTextSize(size);p.setColor(color);p.setTextAlign(Paint.Align.CENTER);
        c.drawText(TextUtils.ellipsize(value,new TextPaint(p),width,TextUtils.TruncateAt.END).toString(),x,y,p);p.setTextAlign(Paint.Align.LEFT);
    }
    private static void button(Canvas c,Paint p,int hovered,int pressed,int id,float x,float y,float width,float height,boolean primary,boolean selected) {
        p.setColor(primary ? (pressed==id ? 0xFFABB6C4 : hovered==id ? 0xFFC4CEDB : HorizonStyle.PRIMARY) : pressed==id ? HorizonStyle.PRESSED : hovered==id ? HorizonStyle.HOVER : selected ? HorizonStyle.SELECTED : HorizonStyle.SURFACE_RAISED);
        c.drawRoundRect(x,y,x+width,y+height,height/2,height/2,p);
        if(hovered==id) {p.setStyle(Paint.Style.STROKE);p.setStrokeWidth(2);p.setColor(HorizonStyle.ACCENT);c.drawRoundRect(x,y,x+width,y+height,height/2,height/2,p);p.setStyle(Paint.Style.FILL);}
    }
    private static void icon(Canvas c,Paint p,String icon,float x,float y,int color) {
        p.setColor(color);
        if(icon.equals("pause")) {c.drawRoundRect(x-10,y-12,x-3,y+12,2,2,p);c.drawRoundRect(x+3,y-12,x+10,y+12,2,2,p);return;}
        if(icon.equals("play")) {Path a=new Path();a.moveTo(x-7,y-13);a.lineTo(x+13,y);a.lineTo(x-7,y+13);a.close();c.drawPath(a,p);return;}
        p.setStrokeWidth(3);p.setStrokeCap(Paint.Cap.ROUND);
        if(icon.equals("close")) {c.drawLine(x-9,y-9,x+9,y+9,p);c.drawLine(x+9,y-9,x-9,y+9,p);}
        else {c.drawLine(x-10,y-4,x,y+6,p);c.drawLine(x,y+6,x+10,y-4,p);}
    }
    private static void seek(Canvas c,Paint p,float x,float y,boolean forward) {
        p.setColor(HorizonStyle.TEXT);p.setStyle(Paint.Style.STROKE);p.setStrokeWidth(2.5f);
        c.drawArc(x-17,y-17,x+17,y+17,forward ? 210 : -30,290,false,p);p.setStyle(Paint.Style.FILL);
        float sign=forward ? 1 : -1;Path arrow=new Path();arrow.moveTo(x+sign*18,y-17);arrow.lineTo(x+sign*18,y-6);arrow.lineTo(x+sign*8,y-11);arrow.close();c.drawPath(arrow,p);
        centered(c,p,"10",x,y+6,16,HorizonStyle.TEXT,32);
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
