package org.questjellyfin.player;

import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Bounded random-access cache for interleaved high-bitrate media; no credentials in diagnostics. */
final class HttpRangeReader implements Closeable {
    private static final Pattern CONTENT_RANGE = Pattern.compile("bytes (\\d+)-(\\d+)/(\\d+)");
    private final URL url;
    private final String authorization;
    private final int pageSize, pageCount;
    private final LinkedHashMap<Long, byte[]> pages = new LinkedHashMap<>(16, .75f, true);
    private volatile boolean closed;
    private volatile HttpURLConnection active;
    private long length = -1;
    private volatile long loads, bytesLoaded, loadMillis, cacheHits;
    String diagnostics() { return "loads="+loads+" bytes="+bytesLoaded+" fetchMs="+loadMillis+" hits="+cacheHits; }

    HttpRangeReader(String url, String authorization) throws IOException {
        this(url, authorization, 8 * 1024 * 1024, 16);
    }
    HttpRangeReader(String address, String authorization, int pageSize, int pageCount) throws IOException {
        url = new URL(address);
        if (!(url.getProtocol().equals("https") || url.getProtocol().equals("http")) || url.getUserInfo() != null)
            throw new IOException("Unsupported media address");
        if (authorization == null || authorization.indexOf('\r') >= 0 || authorization.indexOf('\n') >= 0)
            throw new IOException("Invalid media authorization");
        if (pageSize <= 0 || pageCount <= 0) throw new IllegalArgumentException("Invalid media cache bounds");
        this.authorization = authorization; this.pageSize = pageSize; this.pageCount = pageCount;
    }
    synchronized long size() throws IOException {
        ensureOpen();
        if (length < 0) page(0);
        return length;
    }
    synchronized int readAt(long position, byte[] buffer, int offset, int size) throws IOException {
        ensureOpen();
        if (position < 0 || offset < 0 || size < 0 || offset > buffer.length - size)
            throw new IllegalArgumentException("Invalid media read bounds");
        if (size == 0) return 0;
        if (length < 0) page(0);
        if (length >= 0 && position >= length) return -1;
        int copied = 0;
        while (copied < size && (length < 0 || position < length)) {
            long index = position / pageSize;
            byte[] page = page(index);
            int start = (int)(position % pageSize), available = Math.min(size - copied, page.length - start);
            if (available <= 0) break;
            System.arraycopy(page, start, buffer, offset + copied, available);
            position += available; copied += available;
        }
        return copied == 0 ? -1 : copied;
    }
    private byte[] page(long index) throws IOException {
        ensureOpen();
        byte[] cached = pages.get(index);
        if (cached != null) { cacheHits++; return cached; }
        if (index > (Long.MAX_VALUE - pageSize) / pageSize) throw new IOException("Media offset exceeds supported range");
        long start = index * pageSize, end = start + pageSize - 1;
        HttpURLConnection connection = (HttpURLConnection)url.openConnection();
        connection.setConnectTimeout(10000); connection.setReadTimeout(10000);
        connection.setInstanceFollowRedirects(false);
        connection.setRequestProperty("Authorization", authorization);
        connection.setRequestProperty("Accept-Encoding", "identity");
        connection.setRequestProperty("Range", "bytes=" + start + "-" + end);
        active = connection;
        long began = System.nanoTime();
        boolean complete = false;
        try {
            ensureOpen();
            int status = connection.getResponseCode();
            if (status != 206) throw new IOException("Media range request rejected (HTTP " + status + ")");
            String encoding = connection.getContentEncoding();
            if (encoding != null && !encoding.equalsIgnoreCase("identity")) throw new IOException("Encoded media range is unsupported");
            Matcher range = CONTENT_RANGE.matcher(String.valueOf(connection.getHeaderField("Content-Range")));
            if (!range.matches()) throw new IOException("Media server omitted its byte range");
            long actualStart, actualEnd, total;
            try { actualStart = Long.parseLong(range.group(1)); actualEnd = Long.parseLong(range.group(2)); total = Long.parseLong(range.group(3)); }
            catch (NumberFormatException e) { throw new IOException("Invalid media range size"); }
            if (actualStart != start || actualEnd < start || actualEnd != Math.min(end, total - 1) || total <= actualEnd || (length >= 0 && length != total))
                throw new IOException("Media byte range changed or did not match the request");
            int expected = (int)(actualEnd - actualStart + 1);
            byte[] bytes = new byte[expected];
            try (InputStream input = connection.getInputStream()) {
                int count = 0;
                while (count < bytes.length) {
                    ensureOpen();
                    int read = input.read(bytes, count, bytes.length - count);
                    if (read < 0) throw new IOException("Truncated media byte range");
                    count += read;
                }
            }
            ensureOpen(); length = total;
            loads++; bytesLoaded += bytes.length; loadMillis += (System.nanoTime()-began)/1000000;
            complete = true;
            // Evict before insertion to keep retained cache memory within the bound.
            if (pages.size() == pageCount) pages.remove(pages.keySet().iterator().next());
            pages.put(index, bytes);
            return bytes;
        } finally {
            if (active == connection) active = null;
            if (!complete) connection.disconnect();
        }
    }
    private void ensureOpen() throws IOException { if (closed) throw new IOException("Media source is closed"); }
    @Override public void close() {
        closed = true;
        HttpURLConnection connection = active;
        if (connection != null) connection.disconnect();
    }
}
