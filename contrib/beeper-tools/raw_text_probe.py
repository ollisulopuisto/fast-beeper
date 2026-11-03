#!/usr/bin/env python3
"""Show how link separators arrive in received messages. Read-only.

Diagnoses whether a link in a message's `text` holds a bare ampersand or an
escaped one (U+0026 followed by a, m, p, semicolon). It searches with the
documented GET /v1/messages/search, tries up to five phrases in order until one
hits, and keeps at most six hits. For each hit it prints only a structural
summary of the links: scheme and host, path segment lengths, each query
parameter's name, its value length (values of one or two characters are shown),
and the exact separator bytes between parameters. It also prints the payload
field names, the message type, whether the mapper's HTML branch would be taken
(same tag rule as plain_rich_text in src/backend/beeper.rs), and counts of bare
versus entity ampersands in the whole text. It never prints keys, file names,
other text, the token, or chat, sender or account IDs, and it never sends or
marks anything read. The token comes from BEEPER_ACCESS_TOKEN or a local
no-echo prompt, never from an argument.
"""
import argparse
import getpass
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from run_client import validate_endpoint, LOOPBACK

MAX_HITS = 6
MAX_QUERIES = 5
# Phrases that identify the reported message without printing anything of it.
DEFAULT_PHRASES = ('hh_uunista_ulos.mp4', 'dropbox.com/scl/fi/s2blo6', 'uunista_ulos', 'dropbox.com/scl/fi', 'rlkey=')
# The tag names plain_rich_text treats as markup (src/backend/beeper.rs).
KNOWN_TAGS = ('p', 'div', 'br', 'html', 'span', 'a', 'strong', 'b', 'ul', 'ol', 'li', 'blockquote', 'script',
              'style', 'iframe', 'object', 'em', 'i', 'img', 'h1', 'h2', 'h3', 'tr')
ENTITY = re.compile(r'&(?:#[0-9]+|#[xX][0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);')
URL = re.compile(r'(?:https?://|www\.)[^\s<>"\']+', re.I)
SEPARATOR = re.compile(r'(&amp;|&#38;|&#x26;|&|;)')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Endpoint redirected; no token was forwarded.')


def looks_html(text):
    """The exact condition plain_rich_text uses to take its HTML branch."""
    lower = text.lower()
    for start, char in enumerate(lower):
        if char != '<':
            continue
        tail = lower[start + 1:].lstrip('/')
        for name in KNOWN_TAGS:
            if tail.startswith(name):
                rest = tail[len(name):]
                if rest.startswith(('>', '/')) or (rest[:1] and rest[0].isspace()):
                    return True
    return False


def shown(value):
    return value if len(value) <= 2 else '<%d chars>' % len(value)


def link_shape(url):
    """Structure of one link with every private value masked."""
    if not re.match(r'https?://', url, re.I):
        url = 'https://' + url
    parts = urllib.parse.urlsplit(url)
    scheme_host, path, query = parts.scheme + '://' + parts.netloc, parts.path.strip('/'), parts.query
    params = []
    if query:
        parts = SEPARATOR.split(query)
        for index in range(0, len(parts), 2):
            name, _, value = parts[index].partition('=')
            separator = parts[index + 1] if index + 1 < len(parts) else ''
            params.append({'name_length': len(name),
                           'name': name if re.fullmatch(r'[A-Za-z_]{1,12}', name) else '<masked>',
                           'value': shown(value),
                           'separator_after': json.dumps(separator),
                           'separator_after_hex': separator.encode('utf-8').hex(' ')})
    return {'scheme_host': scheme_host if len(scheme_host) <= 40 else '<masked>',
            'path_segment_lengths': [len(part) for part in path.split('/')] if path else [],
            'params': params}


def describe(item, phrase_number):
    text = item.get('text') or ''
    bare = len(re.findall(r'&(?!(?:#[0-9]+|#[xX][0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);)', text))
    return {'matched_phrase_number': phrase_number,
            'payload_fields': sorted(k for k in item.keys() if isinstance(k, str)),
            'type': item.get('type') or '',
            'mapper_takes_html_branch': looks_html(text),
            'text_length': len(text),
            'bare_ampersands_in_text': bare,
            'entity_ampersands_in_text': len(ENTITY.findall(text)),
            'links': [link_shape(url) for url in URL.findall(text)]}


def probe(search, phrases=DEFAULT_PHRASES):
    """`search(query)` returns the decoded search response."""
    tried = 0
    for number, phrase in enumerate(phrases, 1):
        if tried == MAX_QUERIES:
            break
        tried += 1
        response = search(phrase)
        items = response.get('items', []) if isinstance(response, dict) else []
        hits = []
        for item in items if isinstance(items, list) else []:
            if isinstance(item, dict) and phrase.lower() in (item.get('text') or '').lower():
                hits.append(describe(item, number))
            if len(hits) == MAX_HITS:
                break
        if hits:
            return {'read_only': True, 'queries_tried': tried, 'hit_count': len(hits), 'hits': hits}
    return {'read_only': True, 'queries_tried': tried, 'hit_count': 0, 'hits': []}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:23373')
    parser.add_argument('--phrase', action='append', help='Replace the built-in phrases (8+ characters each, at most five)')
    args = parser.parse_args()
    phrases = tuple(args.phrase) if args.phrase else DEFAULT_PHRASES
    if len(phrases) > MAX_QUERIES or any(len(p) < 5 for p in phrases):
        parser.error('Use up to five phrases of at least 5 characters')
    try:
        parsed = validate_endpoint(args.base_url)
        token = os.environ.get('BEEPER_ACCESS_TOKEN') or getpass.getpass('Beeper API token (local terminal only): ')
        handlers = [NoRedirect()]
        if parsed.hostname in LOOPBACK:
            handlers.append(urllib.request.ProxyHandler({}))
        opener = urllib.request.build_opener(*handlers)

        def search(query):
            url = args.base_url.rstrip('/') + '/v1/messages/search?' + urllib.parse.urlencode({'query': query, 'limit': 20})
            request = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + token})
            with opener.open(request, timeout=20) as response:
                body = response.read(8 * 1024 * 1024 + 1)
                if len(body) > 8 * 1024 * 1024:
                    raise ValueError('response exceeded size limit')
                return json.loads(body)
        print(json.dumps(probe(search, phrases), indent=2))
    except urllib.error.HTTPError as exc:
        print('Endpoint returned HTTP %s. Check URL and token locally.' % exc.code, file=sys.stderr)
        return 1
    except Exception as exc:
        print('Probe failed (%s). Check local connectivity and API configuration.' % type(exc).__name__, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
