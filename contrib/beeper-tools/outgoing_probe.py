#!/usr/bin/env python3
"""Show why outgoing (own) messages may be invisible. Read-only.

Diagnoses "my own messages don't show" for one chat the user picks. It lists
recent chats (titles shown only on the local terminal, never in the JSON),
then walks bounded history pages of the chosen chat - first page plus older
pages via the cursor contract the app itself uses (cursor=oldestCursor,
direction=before, camelCase hasMore/oldestCursor) - and prints, per message,
only structure: direction (outgoing/incoming), the isHidden and isDeleted
flags, the message type, text length, and whether the app's filter (drop
isHidden or isDeleted) would show it. The summary counts outgoing messages
across all fetched pages, records pagination stalls (hasMore with a missing
or repeated cursor), and names the verdict: outgoing absent from the fetched
pages, suppressed by a flag, or clean (server looks right; bug is
downstream). "Absent" is only claimed over the pages actually fetched; the
summary states whether older history remained unfetched. It never prints
message text, ids of any kind, the token, or participant entries, and it
never sends or marks anything read. The token comes from
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

MAX_PAGES = 10
PICK_LIST_SIZE = 15


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


def resolve_chat(get, chat_id, pick):
    """The chat to inspect: explicit id, or an interactive local pick."""
    if chat_id is not None:
        return 'given', chat_id
    chats = get('/v1/chats', {}).get('items', [])
    chats = [c for c in chats if isinstance(c, dict) and c.get('id')]
    if not chats:
        return 'error', None
    print('Recent chats (local only; nothing of this is in the JSON output):',
          file=sys.stderr)
    for index, chat in enumerate(chats[:PICK_LIST_SIZE], 1):
        print('%2d. %s [%s] %s' % (index, chat.get('title') or '(untitled)',
                                   chat.get('network') or '?',
                                   chat.get('lastActivity') or ''),
              file=sys.stderr)
    choice = pick()
    try:
        number = int(choice)
    except (TypeError, ValueError):
        raise ValueError('Not a chat number.')
    if not 1 <= number <= min(len(chats), PICK_LIST_SIZE):
        raise ValueError('Chat number out of range.')
    return 'interactive-pick', chats[number - 1].get('id')


def fetch_pages(get, chat_id, max_pages):
    """Newest page plus older pages via the app's cursor contract."""
    messages = []
    cursor = None
    pages = 0
    stalled = None
    complete = False
    while pages < max_pages:
        query = {}
        if cursor:
            query = {'cursor': cursor, 'direction': 'before'}
        page = get('/v1/chats/%s/messages' % urllib.parse.quote(chat_id, safe=''),
                   query)
        items = page.get('items', []) if isinstance(page, dict) else []
        messages.extend(m for m in items if isinstance(m, dict))
        pages += 1
        more = page.get('hasMore') is True
        if not more:
            complete = True
            break
        next_cursor = page.get('oldestCursor')
        if not next_cursor:
            stalled = 'missing-cursor'
            break
        if next_cursor == cursor:
            stalled = 'repeated-cursor'
            break
        cursor = next_cursor
    return messages, pages, complete, stalled


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
    }


def verdict(items, pages, complete):
    """Name the failure mode from the fetched pages' flags."""
    outgoing = [m for m in items if isinstance(m, dict) and m.get('isSender')]
    scope = '%d-pages%s' % (pages, '' if complete else '-partial-history')
    if not outgoing:
        return 'outgoing-absent-from-' + scope
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
    return 'outgoing-clean-in-' + scope


def probe(get, chat_id, max_pages, limit, pick=None, now=None):
    """`get(path, query)` returns the decoded response."""
    now = now if now is not None else int(datetime.datetime.now(
        datetime.timezone.utc).timestamp())
    selected, chat_id = resolve_chat(get, chat_id, pick or (lambda: None))
    if chat_id is None:
        return {'read_only': True, 'error': 'no chats'}
    max_pages = max(1, min(max_pages, MAX_PAGES))
    items, pages, complete, stalled = fetch_pages(get, chat_id, max_pages)
    items.sort(key=lambda m: parse_time(m.get('timestamp')) or 0, reverse=True)
    outgoing = [m for m in items if m.get('isSender')]
    result = {
        'read_only': True,
        'chat_selected': selected,
        'pages_fetched': pages,
        'history_complete': complete,
        'messages_in_pages': len(items),
        'outgoing_in_pages': len(outgoing),
        'outgoing_app_would_show': sum(
            1 for m in outgoing
            if m.get('isHidden') is not True and m.get('isDeleted') is not True),
        'unknown_direction': sum(1 for m in items if m.get('isSender') is None),
        'verdict': verdict(items, pages, complete),
        'messages': [summarize(m, now) for m in items[:limit]],
    }
    if stalled:
        result['pagination_stalled'] = stalled
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    parser.add_argument('--chat-id', default=None,
                        help='Chat to inspect; default is an interactive pick from recent chats')
    parser.add_argument('--pages', type=int, default=3,
                        help='History pages to walk (1-10, default 3)')
    parser.add_argument('--limit', type=int, default=30,
                        help='How many messages to list in the output')
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
        print(json.dumps(probe(get, args.chat_id, args.pages, args.limit,
                               pick=lambda: input('Chat number to inspect: ')), indent=2))
    except urllib.error.HTTPError as exc:
        print('Endpoint returned HTTP %s. Check URL and token locally.' % exc.code, file=sys.stderr)
        return 1
    except Exception as exc:
        print('Probe failed (%s). Check local connectivity and API configuration.' % type(exc).__name__, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
