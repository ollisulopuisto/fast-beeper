#!/usr/bin/env python3
"""Show what delivery/read receipt fields the bridge reports. Read-only.

Diagnoses "read receipts not working": whether the API's recent outgoing
messages carry sendStatus/seen at all, and which Delivery state the mapper
(src/backend/beeper.rs delivery_of/read_state) derives from each. It fetches
the newest messages of one chat - --chat-id, or the chat with the latest
activity when omitted - and prints, per outgoing message, only structure: the
status string, how many users delivery lists, the shape of `seen` (absent,
bool, timestamp, or a per-participant map and how many entries carry times),
and the resulting tick state. It never prints message text, ids of any kind,
the token, or participant entries, and it never sends or marks anything read.
The token comes from BEEPER_ACCESS_TOKEN or a local no-echo prompt, never from
an argument.
"""
import argparse
import datetime
import getpass
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
try:  # in the repo, share the launcher's endpoint rules
    from run_client import validate_endpoint, LOOPBACK
except ImportError:  # a lone copy of this file keeps the same rules
    LOOPBACK = ('localhost', '127.0.0.1', '::1')

    def validate_endpoint(base):
        parsed = urllib.parse.urlsplit(base)
        if (not parsed.hostname or parsed.username or parsed.password
                or parsed.scheme not in ('https', 'http') or parsed.query or parsed.fragment
                or parsed.path not in ('', '/')):
            raise ValueError('Use the existing API server root URL.')
        if parsed.scheme == 'http' and parsed.hostname not in LOOPBACK:
            raise ValueError('Remote HTTP is refused. Use HTTPS or loopback.')
        _ = parsed.port
        return parsed

MAX_LIMIT = 50


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Endpoint redirected; no token was forwarded.')


def parse_time(value):
    """A receipt timestamp string as a Unix second, or None."""
    if not isinstance(value, str) or not value:
        return None
    try:
        return int(datetime.datetime.fromisoformat(
            value.replace('Z', '+00:00')).timestamp())
    except ValueError:
        return None


def seen_shape(seen, sender_known):
    """The structure of a `seen` value, with every participant masked."""
    if seen is None:
        return {'shape': 'absent'}
    if isinstance(seen, bool):
        return {'shape': 'bool', 'value': seen}
    if isinstance(seen, str):
        return {'shape': 'string', 'is_timestamp': parse_time(seen) is not None}
    if isinstance(seen, dict):
        entries = list(seen.values())
        with_time = sum(1 for entry in entries if parse_time(entry) is not None)
        return {'shape': 'map', 'entries': len(entries), 'entries_with_time': with_time}
    return {'shape': type(seen).__name__}


def delivery_of(item):
    """The mapper's Delivery for one outgoing message, in words."""
    seen = item.get('seen')
    if isinstance(seen, bool):
        return 'read' if seen else 'sent'
    if isinstance(seen, str) and seen:
        return 'read'
    if isinstance(seen, dict):
        def read(value):
            return (value is True) or (isinstance(value, str) and bool(value)) \
                or (isinstance(value, dict) and any(read(v) for v in value.values()))
        if any(read(v) for k, v in seen.items()):
            return 'read-by-some'
    status = item.get('sendStatus')
    if isinstance(status, dict):
        text = status.get('status') or ''
        if text.startswith('FAIL_'):
            return 'failed'
        if text == 'PENDING':
            return 'pending'
        if status.get('deliveredToUsers'):
            return 'delivered'
        if text == 'SUCCESS':
            return 'sent'
    return 'unknown'


def summarize(item, now):
    """One outgoing message's receipt evidence, structure only."""
    timestamp = parse_time(item.get('timestamp'))
    return {
        'minutes_ago': round((now - timestamp) / 60) if timestamp else None,
        'send_status': (item.get('sendStatus') or {}).get('status'),
        'delivered_to_count': len((item.get('sendStatus') or {}).get('deliveredToUsers') or []),
        'seen': seen_shape(item.get('seen'), True),
        'mapper_shows': delivery_of(item),
        'payload_fields': sorted(k for k in item.keys() if isinstance(k, str)),
    }


def probe(get, chat_id, limit, now=None):
    """`get(path, query)` returns the decoded response."""
    now = now if now is not None else int(datetime.datetime.now(
        datetime.timezone.utc).timestamp())
    if chat_id is None:
        chats = get('/v1/chats', {}).get('items', [])
        if not chats:
            return {'read_only': True, 'error': 'no chats'}
        chat = max(chats, key=lambda c: parse_time(c.get('lastActivity')) or 0)
        chat_id = chat.get('id')
    page = get('/v1/chats/%s/messages' % urllib.parse.quote(chat_id, safe=''),
               {'limit': min(limit, MAX_LIMIT)})
    items = page.get('items', []) if isinstance(page, dict) else []
    outgoing = [m for m in items if isinstance(m, dict) and m.get('isSender')]
    outgoing.sort(key=lambda m: parse_time(m.get('timestamp')) or 0, reverse=True)
    return {
        'read_only': True,
        'chat_selected': 'latest-activity' if chat_id else 'given',
        'outgoing_in_page': len(outgoing),
        'messages': [summarize(m, now) for m in outgoing[:limit]],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    parser.add_argument('--chat-id', default=None,
                        help='Chat to inspect; default is the chat with the latest activity')
    parser.add_argument('--limit', type=int, default=10)
    args = parser.parse_args()
    try:
        parsed = validate_endpoint(args.base_url)
        token = os.environ.get('BEEPER_ACCESS_TOKEN') or getpass.getpass('Beeper API token (local terminal only): ')
        handlers = [NoRedirect()]
        if parsed.hostname in LOOPBACK:
            handlers.append(urllib.request.ProxyHandler({}))
        opener = urllib.request.build_opener(*handlers)

        def get(path, query):
            url = args.base_url.rstrip('/') + path
            if query:
                url += '?' + urllib.parse.urlencode(query)
            request = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + token})
            with opener.open(request, timeout=20) as response:
                body = response.read(8 * 1024 * 1024 + 1)
                if len(body) > 8 * 1024 * 1024:
                    raise ValueError('response exceeded size limit')
                return json.loads(body)
        print(json.dumps(probe(get, args.chat_id, args.limit), indent=2))
    except urllib.error.HTTPError as exc:
        print('Endpoint returned HTTP %s. Check URL and token locally.' % exc.code, file=sys.stderr)
        return 1
    except Exception as exc:
        print('Probe failed (%s). Check local connectivity and API configuration.' % type(exc).__name__, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
