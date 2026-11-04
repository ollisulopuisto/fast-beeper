#!/usr/bin/env python3
"""Show what poll data the bridge reports, if any. Read-only.

Diagnoses "polls don't work": the documented Desktop API has no poll
content, no vote endpoint, and no poll in the send body - only a per-chat
capabilities.poll level. This probe checks what actually arrives: the
message type histogram of a chat's recent messages, any payload keys the
documented schema does not list (key names only, never values), the
structural shape of any poll-typed message, and the poll capability level
of each chat. It never prints message text, ids of any kind, the token, or
option/question strings, and it never sends or marks anything read. The
token comes from BEEPER_ACCESS_TOKEN or a local no-echo prompt, never from
an argument.
"""
import argparse
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

# Message payload keys listed in the Desktop API reference (Messages
# resource). Anything beyond these is undocumented and interesting here.
DOCUMENTED_MESSAGE_KEYS = frozenset({
    'id', 'accountID', 'chatID', 'senderID', 'sortKey', 'timestamp',
    'attachments', 'editedTimestamp', 'isDeleted', 'isHidden', 'isSender',
    'isUnread', 'linkedMessageID', 'links', 'mentions', 'reactions',
    'seen', 'senderName', 'sendStatus', 'text', 'type',
})

# Poll-ish payload keys to describe structurally when present.
POLL_MARKERS = ('question', 'options', 'answers', 'votes', 'poll')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Endpoint redirected; no token was forwarded.')


def undocumented_keys(item):
    """Payload keys outside the documented schema; names only."""
    return sorted(k for k in item if isinstance(k, str) and k not in DOCUMENTED_MESSAGE_KEYS)


def type_histogram(items):
    """Message type counts; absent types reported as such."""
    counts = {}
    for item in items:
        if not isinstance(item, dict):
            continue
        kind = item.get('type')
        kind = kind if isinstance(kind, str) and kind else 'absent'
        counts[kind] = counts.get(kind, 0) + 1
    return counts


def value_shape(value):
    """The shape of one value with every string masked to its length."""
    if isinstance(value, dict):
        return {'shape': 'map', 'keys': sorted(value.keys())}
    if isinstance(value, list):
        return {'shape': 'list', 'entries': len(value)}
    if isinstance(value, str):
        return {'shape': 'string', 'length': len(value)}
    return {'shape': type(value).__name__}


def looks_pollish(item):
    kind = item.get('type')
    if isinstance(kind, str) and kind.upper() == 'POLL':
        return True
    return any('poll' in k.lower() for k in item if isinstance(k, str))


def poll_shape(item):
    """One poll-ish message's structure: key names and counts, no content."""
    markers = {k: value_shape(v) for k, v in item.items()
               if isinstance(k, str) and any(m in k.lower() for m in POLL_MARKERS)}
    return {
        'type': item.get('type'),
        'undocumented_keys': undocumented_keys(item),
        'poll_markers': markers,
    }


def capability_summary(chats):
    """Poll capability level per network, as chat counts; no chat ids."""
    summary = {}
    for chat in chats:
        if not isinstance(chat, dict):
            continue
        network = chat.get('network') or 'unknown'
        level = (chat.get('capabilities') or {}).get('poll')
        key = (network, 'absent' if level is None else level)
        summary[key] = summary.get(key, 0) + 1
    return [{'network': network, 'poll_level': level, 'chats': count}
            for (network, level), count in sorted(summary.items(), key=str)]


def probe(get, chat_id, limit):
    """`get(path, query)` returns the decoded response."""
    chats = get('/v1/chats', {'limit': MAX_LIMIT}).get('items', [])
    report = {'read_only': True, 'poll_capabilities': capability_summary(chats)}
    if chat_id is None:
        if not chats:
            report['error'] = 'no chats'
            return report
        chat_id = max(chats, key=lambda c: c.get('lastActivity') or '').get('id')
        report['chat_selected'] = 'latest-activity'
    else:
        report['chat_selected'] = 'given'
    page = get('/v1/chats/%s/messages' % urllib.parse.quote(chat_id, safe=''),
               {'limit': min(limit, MAX_LIMIT)})
    items = page.get('items', []) if isinstance(page, dict) else []
    report['message_types'] = type_histogram(items)
    report['undocumented_key_sets'] = sorted({
        tuple(undocumented_keys(m)) for m in items if isinstance(m, dict)} - {()})
    pollish = [m for m in items if isinstance(m, dict) and looks_pollish(m)]
    report['poll_like_messages'] = len(pollish)
    report['poll_like_shapes'] = [poll_shape(m) for m in pollish[:10]]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    parser.add_argument('--chat-id', default=None,
                        help='Chat to inspect; default is the chat with the latest activity')
    parser.add_argument('--limit', type=int, default=MAX_LIMIT)
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
