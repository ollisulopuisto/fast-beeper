import json, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse
from datetime import datetime, timezone

import os

SENDS = []
WORK = os.environ.get('SL_E2E_WORK', '/tmp/sltest')
PORT = int(os.environ.get('SL_STUB_PORT', '23901'))

def mode():
    try: return open(WORK + '/mode').read().strip()
    except FileNotFoundError: return 'normal'

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _send(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code); self.send_header("Content-Type","application/json")
        self.send_header("Content-Length", str(len(body))); self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        p = urlparse(self.path)
        if p.path.startswith("/v1/chats/"):
            return self._send(200, {"id":"chat-1","accountID":"acc-1"})
        if p.path == "/v1/messages/search":
            items = [{"id":s[0],"chatID":"chat-1","accountID":"acc-1","text":s[1],"isSender":True,"timestamp":s[2]} for s in SENDS]
            return self._send(200, {"items":items,"hasMore":False,"oldestCursor":None,"newestCursor":None})
        return self._send(404, {"error":"no"})
    def do_POST(self):
        p = urlparse(self.path)
        if p.path.startswith("/v1/chats/") and p.path.endswith("/messages"):
            n = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(n) or b"{}")
            m = mode()
            if m == "hang": time.sleep(3600)
            if m == "reject": return self._send(500, {"error":"bridge refused"})
            SENDS.append((f"stub-{len(SENDS)+1}", body.get("text",""), datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")))
            return self._send(200, {"pendingMessageID":SENDS[-1][0]})
        return self._send(404, {"error":"no"})

ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
