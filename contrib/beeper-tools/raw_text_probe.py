#!/usr/bin/env python3
"""Show the exact received bytes around one phrase of one message. Read-only.

For diagnosing how a message's `text` arrives (for example whether a link holds
a bare ampersand or an escaped one). It searches with the documented
GET /v1/messages/search, keeps at most three hits and prints only a short window
around the phrase as JSON-escaped text plus hex. It never prints the token,
chat, sender or account IDs, or the rest of the message, and it never marks
anything read or sends anything. The token comes from BEEPER_ACCESS_TOKEN or a
local no-echo prompt, never from an argument.
"""
import argparse
import getpass
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from run_client import validate_endpoint, LOOPBACK

MAX_HITS = 3
BEFORE, AFTER = 24, 8


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Endpoint redirected; no token was forwarded.')


def window(text, phrase):
    """The bytes around the phrase, or None when the phrase is absent."""
    at = text.find(phrase)
    if at < 0:
        return None
    part = text[max(0, at - BEFORE):at + len(phrase) + AFTER]
    return {'escaped_text': json.dumps(part, ensure_ascii=True),
            'utf8_hex': part.encode('utf-8').hex(' '),
            'text_contains_angle_bracket': '<' in text,
            'text_length': len(text)}


def probe(search, phrase):
    """`search(query)` returns the decoded search response."""
    response = search(phrase)
    items = response.get('items', []) if isinstance(response, dict) else []
    hits = []
    for item in items if isinstance(items, list) else []:
        found = window(item.get('text') or '', phrase) if isinstance(item, dict) else None
        if found:
            found['type'] = item.get('type') or ''
            hits.append(found)
        if len(hits) == MAX_HITS:
            break
    return {'read_only': True, 'hits': hits, 'hit_count': len(hits)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phrase', help='Part of the message, at least 8 characters (for example the link start)')
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    args = parser.parse_args()
    if len(args.phrase) < 8:
        parser.error('Use a phrase of at least 8 characters so only that message matches')
    try:
        parsed = validate_endpoint(args.base_url)
        token = os.environ.get('BEEPER_ACCESS_TOKEN') or getpass.getpass('Beeper API token (local terminal only): ')
        handlers = [NoRedirect()]
        if parsed.hostname in LOOPBACK:
            handlers.append(urllib.request.ProxyHandler({}))
        opener = urllib.request.build_opener(*handlers)

        def search(query):
            url = args.base_url.rstrip('/') + '/v1/messages/search?' + urllib.parse.urlencode({'query': query, 'limit': 10})
            request = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + token})
            with opener.open(request, timeout=20) as response:
                body = response.read(8 * 1024 * 1024 + 1)
                if len(body) > 8 * 1024 * 1024:
                    raise ValueError('response exceeded size limit')
                return json.loads(body)
        print(json.dumps(probe(search, args.phrase), indent=2))
    except urllib.error.HTTPError as exc:
        print('Endpoint returned HTTP %s. Check URL and token locally.' % exc.code, file=sys.stderr)
        return 1
    except Exception as exc:
        print('Probe failed (%s). Check local connectivity and API configuration.' % type(exc).__name__, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
