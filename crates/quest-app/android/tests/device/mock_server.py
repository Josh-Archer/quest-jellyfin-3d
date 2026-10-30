"""Deterministic Jellyfin fixture for the isolated Android UI smoke test."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs
import json, struct, zlib, sys, time

video, port_file = sys.argv[1:]
favorite = False
search_failed = False

def poster():
    width, height = 240, 360
    raw = b''.join(b'\x00' + bytes([int(30 + y / 3), 80, 150]) * width for y in range(height))
    def chunk(kind, data): return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')

def movie():
    return {'Id':'movie','Name':'Test Feature','Type':'Movie','Overview':'A fixture feature for device testing.','ProductionYear':2026,'ImageTags':{'Primary':'test-art'},'RunTimeTicks':300000000,'UserData':{'PlaybackPositionTicks':20000000,'IsFavorite':favorite},'MediaStreams':[{'Type':'Video','Codec':'h264','Width':640,'Height':360}]}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def send_bytes(self, data, content_type='application/json', status=200):
        self.send_response(status); self.send_header('Content-Type',content_type); self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
    def send_json(self, data): self.send_bytes(json.dumps(data).encode())
    def do_GET(self):
        global search_failed
        parsed = urlparse(self.path); path = parsed.path; query = parse_qs(parsed.query)
        if path == '/Users/user/Views': self.send_json({'Items':[{'Id':'library','Name':'Fixture Movies'}]})
        elif path == '/Users/user/Items/Resume': self.send_json({'Items':[movie()]})
        elif path == '/Users/user/Items/movie': self.send_json(movie())
        elif path == '/Items':
            if query.get('SearchTerm') == ['fail-once'] and not search_failed:
                search_failed = True; self.send_bytes(b'Fixture unavailable', status=503); return
            kind = query.get('IncludeItemTypes',[''])[0]
            if kind == 'Series': items = [{'Id':'series','Name':'Sample Series','Type':'Series','ImageTags':{'Primary':'art'}}]
            elif kind == 'Episode': items = [{'Id':'episode','Name':'Episode One','Type':'Episode','ParentIndexNumber':1,'IndexNumber':1}]
            else: items = [movie()]
            self.send_json({'Items':items,'TotalRecordCount':len(items)})
        elif '/Images/Primary' in path: self.send_bytes(poster(),'image/png')
        elif path == '/Videos/movie/stream':
            data = open(video,'rb').read(); self.send_bytes(data,'video/mp4')
        else: self.send_bytes(b'{}',status=404)
    def do_POST(self):
        global favorite
        self.rfile.read(int(self.headers.get('Content-Length',0)))
        path = urlparse(self.path).path
        if path == '/Items/movie/PlaybackInfo':
            time.sleep(1)
            self.send_json({'PlaySessionId':'play','MediaSources':[{'Id':'source','SupportsDirectPlay':True,'MediaStreams':movie()['MediaStreams']}]})
        elif path == '/Users/user/FavoriteItems/movie': favorite = True; self.send_json({'IsFavorite':True})
        else: self.send_bytes(b'',status=204)

server = ThreadingHTTPServer(('127.0.0.1',0),Handler)
open(port_file,'w').write(str(server.server_port))
server.serve_forever()
