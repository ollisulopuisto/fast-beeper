#!/usr/bin/env python3
"""Show why outgoing (own) messages may be invisible. Read-only.

Diagnoses "my own messages don't show": for the newest messages of one chat
- --chat-id, or the chat with the latest activity when omitted - it prints,
per message, only structure: direction (outgoing/incoming), the isHidden and
isDeleted flags, the message type, whether text is present and its length,
and whether the app's filter (drop isHidden or isDeleted, map the rest) would
show it. A summary counts outgoing messages in the page, how many the app
would show, and names the verdict: outgoing absent from the page, suppressed
by a flag, or clean (the server looks right; the bug is downstream). It never
prints message text, ids of any kind, the token, or participant entries, and
it never sends or marks anything read. The token comes from
BEEPER_ACCESS_TOKEN or a local no-echo prompt, never from an argument.
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
    """A message timestamp string as a Unix second, or None."""
    if not isinstance(value, str) or not value:
        return None
    try:
        return int(datetime.datetime.fromisoformat(
            value.replace('Z', '+00:00')).timestamp())
    except ValueError:
        return None


def summarize(item, now):
    """One message's visibility evidence, structure only."""
    timestamp = parse_time(item.get('timestamp'))
    is_sender = item.get('isSender')
    hidden = item.get('isHidden') is True
    deleted = item.get('isDeleted') is True
    text = item.get('text')
    return {
        'minutes_ago': round((now - timestamp) / 60) if timestamp else None,
        'direction': 'outgoing' if is_sender else ('incoming' if is_sender is False else 'unknown'),
        'is_hidden': hidden,
        'is_deleted': deleted,
        'type': item.get('type'),
        'text_length': len(text) if isinstance(text, str) else 0,
        'app_would_show': not (hidden or deleted),
        'payload_fields': sorted(k for k in item.keys() if isinstance(k, str)),
    }


def verdict(items):
    """Name the failure mode from one page's flags."""
    outgoing = [m for m in items if isinstance(m, dict) and m.get('isSender')]
    if not outgoing:
        return 'outgoing-absent-from-page'
    suppressed = [m for m in outgoing
                  if m.get('isHidden') is True or m.get('isDeleted') is True]
    if suppressed:
        flags = set()
        for m in suppressed:
            if m.get('isHidden') is True:
                flags.add('isHidden')
            if m.get('isDeleted') is True:
                flags.add('isDeleted')
        return 'outgoing-suppressed-by-' + '-and-'.join(sorted(flags))
    return 'outgoing-clean-in-page'


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
    items = [m for m in items if isinstance(m, dict)]
    items.sort(key=lambda m: parse_time(m.get('timestamp')) or 0, reverse=True)
    outgoing = [m for m in items if m.get('isSender')]
    return {
        'read_only': True,
        'chat_selected': 'latest-activity' if chat_id else 'given',
        'page_size': len(items),
        'outgoing_in_page': len(outgoing),
        'outgoing_app_would_show': sum(
            1 for m in outgoing
            if m.get('isHidden') is not True and m.get('isDeleted') is not True),
        'verdict': verdict(items),
        'messages': [summarize(m, now) for m in items[:limit]],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    parser.add_argument('--chat-id', default=None,
                        help='Chat to inspect; default is the chat with the latest activity')
    parser.add_argument('--limit', type=int, default=20)
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
