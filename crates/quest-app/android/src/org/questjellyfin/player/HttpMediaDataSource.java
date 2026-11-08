package org.questjellyfin.player;
import android.media.MediaDataSource;
import java.io.IOException;

/** Public Android media API: avoids the platform HTTP cache's destructive range jumps. */
final class HttpMediaDataSource extends MediaDataSource {
    private final HttpRangeReader reader;
    HttpMediaDataSource(String url, String authorization) throws IOException { reader = new HttpRangeReader(url, authorization); }
    String diagnostics() { return reader.diagnostics(); }
    @Override public int readAt(long position, byte[] buffer, int offset, int size) throws IOException { return reader.readAt(position,buffer,offset,size); }
    @Override public long getSize() throws IOException { return reader.size(); }
    @Override public void close() { reader.close(); }
}
