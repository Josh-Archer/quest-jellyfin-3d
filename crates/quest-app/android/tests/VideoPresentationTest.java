package org.questjellyfin.player;
public class VideoPresentationTest {
    static void state(VideoPresentation p,int expected){if(p.state()!=expected)throw new AssertionError("expected "+expected+" got "+p.state());}
    public static void main(String[] args){
        VideoPresentation p=new VideoPresentation();state(p,0);
        p.buffering(true);state(p,2);p.buffering(false);state(p,0); // Buffer end may precede first layer.
        p.firstFrame();state(p,1);p.buffering(true);state(p,2);p.buffering(false);state(p,1);
        p.seeking(true);p.buffering(true);p.seeking(false);state(p,2);p.buffering(false);state(p,1);
        p.observePosition(100,true,1000);p.observePosition(100,true,2600);state(p,2);
        p.observePosition(101,true,2700);state(p,1);p.observePosition(101,false,8000);state(p,1);
        p.observePosition(101,false,12000);state(p,1); // User pause is not a buffer stall.
    }
}
