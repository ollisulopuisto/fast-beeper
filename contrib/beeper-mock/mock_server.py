#!/usr/bin/env python3
"""Mock Beeper Client API for the beeper-backend demo: no Beeper account needed.

Run:  uv run contrib/beeper-mock/mock_server.py
Then: ZAPFAST_BACKEND=beeper \
      BEEPER_DESKTOP_BASE_URL=http://127.0.0.1:8077 \
      BEEPER_ACCESS_TOKEN=mock \
      cargo run

Serves two accounts on the SAME network (WhatsApp x2) plus Slack, so the
multiple-account labelling is visible, plus paginated history and live
message injection for the "Family" chat (a new message arrives ~20s after
startup and after every sent message).
"""

import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs, unquote

START = time.time()

CHATS = [
    {
        "id": "!famchat:beeper.local",
        "accountID": "local-whatsapp*ba*111",
        "network": "WhatsApp",
        "title": "Family",
        "type": "group",
        "unreadCount": 3,
        "isPinned": True,
        "lastActivity": "2026-10-27T14:02:11.000Z",
        "participants": {"hasMore": False, "items": [{"id": "@alice:beeper.local"}, {"id": "@bob:beeper.local"}], "total": 5},
        "preview": {
            "id": "m-fam-9", "chatID": "!famchat:beeper.local",
            "accountID": "local-whatsapp*ba*111",
            "senderID": "@alice:beeper.local", "senderName": "Alice",
            "sortKey": "0000019000000009", "timestamp": "2026-10-27T14:02:11.000Z",
            "text": "Dinner at 19?", "type": "TEXT", "isSender": False,
        },
    },
    {
        "id": "!alice:beeper.local",
        "accountID": "local-whatsapp*ba*222",
        "network": "WhatsApp",
        "title": "Alice (work)",
        "type": "single",
        "unreadCount": 0,
        "lastActivity": "2026-10-26T09:40:00.000Z",
        "participants": {"hasMore": False, "items": [{"id": "@alice:beeper.local"}], "total": 2},
        "preview": {
            "id": "m-al-4", "chatID": "!alice:beeper.local",
            "accountID": "local-whatsapp*ba*222",
            "senderID": "@me:beeper.local",
            "sortKey": "0000018000000004", "timestamp": "2026-10-26T09:40:00.000Z",
            "text": "your flight is at 6:40, gate B23", "type": "TEXT", "isSender": True,
        },
    },
    {
        "id": "!standup:beeper.local",
        "accountID": "slackgo.T01-U01",
        "network": "Slack",
        "title": "Eng standup",
        "type": "group",
        "unreadCount": 12,
        "isMuted": True,
        "lastActivity": "2026-10-27T08:15:00.000Z",
        "participants": {"hasMore": True, "items": [], "total": 14},
    },
]

ACCOUNTS = [
    {"accountID": "local-whatsapp*ba*111", "bridge": {"id": "whatsapp", "provider": "local", "type": "whatsapp"},
     "status": "connected", "user": {"id": "@me:beeper.local", "fullName": "Personal"}},
    {"accountID": "local-whatsapp*ba*222", "bridge": {"id": "whatsapp", "provider": "local", "type": "whatsapp"},
     "status": "connected", "user": {"id": "@me-work:beeper.local", "fullName": "Work"}},
    {"accountID": "slackgo.T01-U01", "bridge": {"id": "slackgo", "provider": "cloud", "type": "slack"},
     "status": "connected", "user": {"id": "@demo:slack", "fullName": "Demo user"}},
]

HISTORY = [
    {"id": "m-fam-6", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@bob:beeper.local", "senderName": "Bob", "sortKey": "0000019000000006",
     "timestamp": "2026-10-27T13:58:02.000Z", "text": "I can pick up the cake on the way",
     "type": "TEXT", "isSender": False},
    {"id": "m-fam-7", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@me:beeper.local", "sortKey": "0000019000000007",
     "timestamp": "2026-10-27T13:59:44.000Z", "text": "Perfect, thanks!",
     "type": "TEXT", "isSender": True},
    {"id": "m-fam-8", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@alice:beeper.local", "senderName": "Alice", "sortKey": "0000019000000008",
     "timestamp": "2026-10-27T14:00:10.000Z", "type": "IMAGE", "isSender": False,
     "attachments": [{"type": "img", "id": "mxc://mock/photo", "fileName": "demo-photo.png", "mimeType": "image/png", "fileSize": 1024, "size": {"width": 400, "height": 240}}]},
    {"id": "m-fam-9", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@alice:beeper.local", "senderName": "Alice", "sortKey": "0000019000000009",
     "timestamp": "2026-10-27T14:02:11.000Z", "text": "Dinner at 19?", "type": "TEXT", "isSender": False},
]

OLDER = [
    {"id": "m-fam-1", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@alice:beeper.local", "senderName": "Alice", "sortKey": "0000018000000001",
     "timestamp": "2026-10-20T18:00:00.000Z", "text": "Who is bringing what on Sunday?",
     "type": "TEXT", "isSender": False},
    {"id": "m-fam-2", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
     "senderID": "@bob:beeper.local", "senderName": "Bob", "sortKey": "0000018000000002",
     "timestamp": "2026-10-20T18:01:12.000Z", "text": "Salad is on me.", "type": "TEXT", "isSender": False},
]

# All networks use the same API contract. Routing is by opaque chat ID.
for net, aid, cid, title in [
    ("Signal", "signal-personal", "!signal:beeper.local", "Signal friend"),
    ("Messenger", "meta-personal", "!messenger:beeper.local", "Messenger friend"),
]:
    ACCOUNTS.append({"accountID": aid, "user": {"id": "@me:" + aid, "fullName": "Demo user"}})
    CHATS.append({"id": cid, "accountID": aid, "network": net, "title": title,
                  "type": "single", "lastActivity": "2026-10-25T12:00:00Z", "unreadCount": 0})
    HISTORY.append({"id": "seed-" + aid, "chatID": cid, "accountID": aid,
                    "senderID": "@friend:" + aid, "senderName": "Friend",
                    "timestamp": "2026-10-25T12:00:00Z", "text": net + " isolated history", "isSender": False})
HISTORY.append({"id": "work-seed", "chatID": "!alice:beeper.local", "accountID": "local-whatsapp*ba*222",
                "senderID": "@work:beeper.local", "timestamp": "2026-10-26T09:40:00Z",
                "text": "Work account isolated history", "isSender": False})

UPLOADS = {}
NEW_MESSAGES = []  # messages sent via POST, plus the scheduled live one
_counter = [100]


def next_message():
    if time.time() - START > 20 and not getattr(next_message, "fired", False):
        next_message.fired = True
        return {"id": "m-live-1", "chatID": "!famchat:beeper.local", "accountID": "local-whatsapp*ba*111",
                "senderID": "@bob:beeper.local", "senderName": "Bob", "sortKey": "0000019000000010",
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime()),
                "text": "Cake secured. See you at 19!", "type": "TEXT", "isSender": False}
    return None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def _json(self, obj, status=200):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = urlparse(self.path).path
        q = parse_qs(urlparse(self.path).query)
        if path == "/v1/info":
            return self._json({"app": {"bundle_id": "com.beeper.mock", "name": "Mock Beeper Server", "version": "0.0.0"},
                               "endpoints": {"mcp": "", "oauth": {"authorization_endpoint": "", "introspection_endpoint": ""},
                                             "spec": "", "ws_events": ""},
                               "platform": {}, "server": {}})
        if path == "/v1/assets/serve":
            data = open(__file__.replace("mock_server.py", "demo-photo.png"), "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        if "/messages/" in path and path.startswith("/v1/chats/"):
            head, message_id = path.rsplit("/messages/", 1)
            chat_id = unquote(head[len("/v1/chats/"):])
            matches = [m for m in HISTORY + OLDER if m["chatID"] == chat_id and m["id"] == unquote(message_id)]
            if matches:
                return self._json(matches[0])
            return self._json({"error": "not found"}, 404)
        if path == "/v1/accounts":
            return self._json(ACCOUNTS)
        if path == "/v1/chats":
            return self._json({"items": CHATS, "hasMore": False, "oldestCursor": None, "newestCursor": None})
        if path == "/v1/chats/search":
            needle = (q.get("query") or [""])[0].lower()
            items = [c for c in CHATS if needle in c["title"].lower()]
            return self._json({"items": items, "hasMore": False, "oldestCursor": None, "newestCursor": None})
        if path == "/v1/messages/search":
            needle = (q.get("query") or [""])[0].lower()
            items = [m for m in HISTORY + OLDER + NEW_MESSAGES if needle in (m.get("text") or "").lower()]
            return self._json({"items": items, "hasMore": False, "oldestCursor": None, "newestCursor": None})
        if path.endswith("/messages") and path.startswith("/v1/chats/"):
            chat_id = unquote(path[len("/v1/chats/"):-len("/messages")])
            direction = (q.get("direction") or [None])[0]
            if direction == "before":
                return self._json({"items": [m for m in OLDER if m["chatID"] == chat_id], "hasMore": False, "oldestCursor": None, "newestCursor": "c-old"})
            if direction == "after":
                live = next_message() if chat_id == "!famchat:beeper.local" else None
                items = [m for m in NEW_MESSAGES if m["chatID"] == chat_id]
                if live and chat_id == "!famchat:beeper.local":
                    items.append(live)
                out = list(items)
                for m in items:
                    if m in NEW_MESSAGES:
                        pass
                # consume sent messages once delivered
                for m in items:
                    if m in NEW_MESSAGES:
                        NEW_MESSAGES.remove(m)
                return self._json({"items": out, "hasMore": False, "oldestCursor": None,
                                   "newestCursor": "c-new-%d" % time.time()} if out else
                                  {"items": [], "hasMore": False, "oldestCursor": None, "newestCursor": None})
            items = [m for m in HISTORY if m["chatID"] == chat_id]
            return self._json({"items": items, "hasMore": bool(items), "oldestCursor": "c-oldest" if items else None,
                               "newestCursor": "c-newest" if items else None})
        return self._json({"code": "not_found", "message": path}, 404)

    def do_POST(self):
        path = urlparse(self.path).path
        length = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(length) or b"{}")
        if path == "/v1/assets/upload/base64":
            import base64
            content = base64.b64decode(body["content"], validate=True)
            upload_id = "fixture-upload-%d" % len(UPLOADS)
            UPLOADS[upload_id] = {"content": content, "fileName": body.get("fileName", "attachment")}
            return self._json({"uploadID": upload_id, "fileName": body.get("fileName", "attachment")})
        if path.endswith("/messages") and path.startswith("/v1/chats/"):
            chat_id = unquote(path[len("/v1/chats/"):-len("/messages")])
            _counter[0] += 1
            msg = {"id": "m-sent-%d" % _counter[0], "chatID": chat_id, "accountID": next(c["accountID"] for c in CHATS if c["id"] == chat_id),
                   "senderID": "@me:beeper.local", "sortKey": "0000020%08d" % _counter[0],
                   "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime()),
                   "text": body.get("text", ""), "type": "TEXT", "isSender": True}
            if body.get("replyToMessageID"):
                msg["linkedMessageID"] = body["replyToMessageID"]
            if body.get("attachment"):
                upload = UPLOADS[body["attachment"]["uploadID"]]
                msg["type"] = "FILE"
                msg["attachments"] = [{"type": "unknown", "id": "mxc://mock/upload", "fileName": upload["fileName"], "fileSize": len(upload["content"]), "mimeType": "application/octet-stream"}]
            chat = next(c for c in CHATS if c["id"] == chat_id)
            chat["preview"] = msg
            HISTORY.append(msg)
            NEW_MESSAGES.append(msg)
            return self._json({"pendingMessageID": msg["id"]})
        if path.endswith("/read"):
            chat_id = unquote(path[len("/v1/chats/"):-len("/read")])
            for chat in CHATS:
                if chat["id"] == chat_id:
                    chat["unreadCount"] = 0
        return self._json({"ok": True})


if __name__ == "__main__":
    print("Mock Beeper Server on http://127.0.0.1:8077 (any bearer token works)")
    ThreadingHTTPServer(("127.0.0.1", 8077), Handler).serve_forever()
