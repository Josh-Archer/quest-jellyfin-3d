package org.questjellyfin.player;
import com.sun.net.httpserver.*;
import java.net.*;import java.io.*;import java.util.*;import java.util.concurrent.*;import java.util.concurrent.atomic.*;
public class HttpRangeReaderTest {
 static byte[] content=new byte[129];static AtomicInteger requests=new AtomicInteger(),redirected=new AtomicInteger();
 static String base;static CountDownLatch entered=new CountDownLatch(1),release=new CountDownLatch(1);
 static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 interface Action{void run()throws Exception;}
 static void fails(Action action)throws Exception{try{action.run();throw new AssertionError("Invalid range was accepted");}catch(IOException expected){check(!expected.toString().contains("private-test-token"),"Credential in error");}}
 public static void main(String[] args)throws Exception{
  for(int i=0;i<content.length;i++)content[i]=(byte)i;
  HttpServer server=HttpServer.create(new InetSocketAddress("127.0.0.1",0),0);
  ExecutorService workers=Executors.newCachedThreadPool();server.setExecutor(workers);
  server.createContext("/",exchange->{try{
   String mode=exchange.getRequestURI().getPath();requests.incrementAndGet();
   if(mode.equals("/sink")){redirected.incrementAndGet();exchange.sendResponseHeaders(403,-1);return;}
   check("private-test-token".equals(exchange.getRequestHeaders().getFirst("Authorization")),"Authorization missing");
   check("identity".equals(exchange.getRequestHeaders().getFirst("Accept-Encoding")),"Compressed range requested");
   if(mode.equals("/redirect")){exchange.getResponseHeaders().set("Location",base+"/sink");exchange.sendResponseHeaders(302,-1);return;}
   if(mode.equals("/stall")){entered.countDown();try{release.await(15,TimeUnit.SECONDS);}catch(InterruptedException e){Thread.currentThread().interrupt();}return;}
   if(mode.equals("/ignored")){exchange.sendResponseHeaders(200,content.length);exchange.getResponseBody().write(content);return;}
   String[] range=exchange.getRequestHeaders().getFirst("Range").substring(6).split("-");
   int start=Integer.parseInt(range[0]),end=Math.min(Integer.parseInt(range[1]),content.length-1);
   if(mode.equals("/bad-start"))start++;
   int total=mode.equals("/changed")&&start>0 ? content.length+1 : content.length;
   String header="bytes "+start+"-"+end+"/"+total;
   if(mode.equals("/overflow"))header="bytes 0-999999999999999999999999/9999999999999999999999999";
   if(mode.equals("/short"))end--;
   else exchange.getResponseHeaders().set("Content-Range",header);
   if(mode.equals("/short"))exchange.getResponseHeaders().set("Content-Range","bytes "+start+"-"+end+"/"+total);
   if(mode.equals("/encoded"))exchange.getResponseHeaders().set("Content-Encoding","gzip");
   exchange.sendResponseHeaders(206,end-start+1);
   exchange.getResponseBody().write(content,start,mode.equals("/truncated")?end-start:end-start+1);
  }finally{exchange.close();}});
  server.start();base="http://127.0.0.1:"+server.getAddress().getPort();
  try{
   requests.set(0);
   try(HttpRangeReader reader=new HttpRangeReader(base+"/ok","private-test-token",16,3)){
    byte[] out=new byte[40];check(reader.size()==129,"Wrong length");
    check(reader.readAt(7,out,3,25)==25,"Cross-page read length");
    for(int i=0;i<25;i++)check(out[3+i]==content[7+i],"Wrong cross-page byte");
    check(requests.get()==2,"Unexpected initial reads");
    reader.readAt(48,out,0,2);reader.readAt(0,out,0,2);reader.readAt(50,out,0,2);
    check(requests.get()==3,"Interleaved tracks lost a cached page");
    reader.readAt(32,out,0,1);reader.readAt(16,out,0,1);
    check(requests.get()==5,"LRU did not evict bounded pages");
    check(reader.readAt(127,out,0,10)==2&&out[0]==127&&out[1]==(byte)128,"Partial EOF read");
    check(reader.readAt(129,out,0,1)==-1&&reader.readAt(129,out,0,0)==0,"EOF contract");
    try{reader.readAt(-1,out,0,1);throw new AssertionError("Negative offset accepted");}catch(IllegalArgumentException expected){}
   }
   try(HttpRangeReader reader=new HttpRangeReader(base+"/ok","private-test-token",16,3)){
    check(reader.readAt(999,new byte[1],0,1)==-1,"Unknown-size EOF contract");reader.close();fails(()->reader.size());
   }
   for(String mode:new String[]{"ignored","bad-start","overflow","short","encoded","truncated","redirect"}){
    try(HttpRangeReader reader=new HttpRangeReader(base+"/"+mode,"private-test-token",16,3)){fails(()->reader.size());}
   }
   check(redirected.get()==0,"Redirect forwarded authorization");
   try(HttpRangeReader reader=new HttpRangeReader(base+"/changed","private-test-token",16,3)){
    reader.size();fails(()->reader.readAt(16,new byte[1],0,1));
   }
   HttpRangeReader stalled=new HttpRangeReader(base+"/stall","private-test-token",16,3);
   Thread pending=new Thread(()->{try{stalled.size();throw new AssertionError("Closed stalled read succeeded");}catch(IOException expected){}});pending.start();
   check(entered.await(3,TimeUnit.SECONDS),"Stalled request never started");long start=System.nanoTime();stalled.close();pending.join(2000);
   check(!pending.isAlive()&&(System.nanoTime()-start)<TimeUnit.SECONDS.toNanos(2),"Close did not cancel pending HTTP read");
   System.out.println("HttpRangeReader: interleaved cache reuse, LRU bound, cross-page reads, EOF, range integrity, auth, redirect refusal and cancellation passed");
  }finally{release.countDown();server.stop(0);workers.shutdownNow();}
 }
}
