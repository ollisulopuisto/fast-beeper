import importlib.util
import json
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('probe_mod', pathlib.Path(__file__).with_name('raw_text_probe.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

KEY = 'SECRETKEY123456'
ESCAPED = 'tein https://www.dropbox.com/scl/fi/abcdef/hh_uunista_ulos.mp4?rlkey=%s&amp;dl=0&SECRETNAME=1 private tail' % KEY
BARE = 'https://www.dropbox.com/scl/fi/abcdef/hh_uunista_ulos.mp4?rlkey=%s&dl=0' % KEY


def search_of(*items):
    return lambda q: {'items': list(items)}


class ProbeTests(unittest.TestCase):
    def test_escaped_separator_is_shown_with_bytes_and_no_private_content(self):
        out = m.probe(search_of({'text': ESCAPED, 'type': 'TEXT', 'chatID': '!room', 'senderID': '@a'}))
        dumped = json.dumps(out)
        for leak in ('SECRETNAME', KEY, 'private tail', '!room', '@a', 'hh_uunista', 'abcdef'):
            self.assertNotIn(leak, dumped)
        params = out['hits'][0]['links_from_text'][0]['params']
        self.assertEqual([p['name'] for p in params], ['rlkey', 'dl', '<masked>'])
        self.assertEqual(params[0]['separator_after_hex'], '26 61 6d 70 3b')
        self.assertEqual(params[1]['dl_value'], '0')
        self.assertEqual(out['hits'][0]['entity_ampersands_in_text'], 1)
        self.assertEqual(out['hits'][0]['bare_ampersands_in_text'], 1)
        self.assertFalse(out['hits'][0]['mapper_takes_html_branch'])
        self.assertEqual(out['hits'][0]['payload_fields'], ['chatID', 'senderID', 'text', 'type'])

    def test_bare_separator_is_distinguished(self):
        out = m.probe(search_of({'text': BARE, 'type': 'TEXT'}))
        hit = out['hits'][0]
        self.assertEqual(hit['links_from_text'][0]['params'][0]['separator_after_hex'], '26')
        self.assertEqual((hit['bare_ampersands_in_text'], hit['entity_ampersands_in_text']), (1, 0))

    def test_html_branch_uses_the_mappers_tag_rule(self):
        self.assertTrue(m.looks_html('<p>hi</p>'))
        self.assertTrue(m.looks_html('x <a href="u">y</a>'))
        self.assertFalse(m.looks_html('a < b && c > d'))
        self.assertFalse(m.looks_html('plain https://x.test/?a=1&amp;b=2'))
        out = m.probe(search_of({'text': '<p>' + BARE + '</p>', 'type': 'TEXT'}))
        self.assertTrue(out['hits'][0]['mapper_takes_html_branch'])

    def test_falls_back_through_phrases_and_bounds_queries_and_hits(self):
        seen = []

        def search(query):
            seen.append(query)
            return {'items': [{'text': BARE}] * 9} if query == 'uunista_ulos' else {'items': []}
        out = m.probe(search)
        self.assertEqual(seen, list(m.DEFAULT_PHRASES))
        self.assertEqual((out['queries_tried'], out['hit_count']), (3, 2))
        out = m.probe(lambda q: {'items': []}, tuple('phrase%d' % i for i in range(9)))
        self.assertEqual(out['queries_tried'], 3)
        self.assertEqual(out['hit_count'], 0)

    def test_payload_link_previews_are_structured_and_masked(self):
        item = {'text': ESCAPED, 'type': 'TEXT',
                'links': [{'title': 'hh_uunista_ulos.mp4 secret name',
                           'url': BARE + '&SECRETNAME=1',
                           'img': 'https://img.test/SECRETKEY123456'}]}
        out = m.probe(search_of(item))
        dumped = json.dumps(out)
        for leak in ('SECRETNAME', KEY, 'secret name', 'hh_uunista', 'img.test'):
            self.assertNotIn(leak, dumped)
        previews = out['hits'][0]['payload_link_previews']
        self.assertEqual(len(previews), 1)
        self.assertTrue(previews[0]['has_title'])
        self.assertIn('url', previews[0]['fields'])
        # The preview url shows its own separator bytes: bare here, so the two
        # sources are distinguishable in one output.
        self.assertEqual(previews[0]['url']['params'][0]['separator_after_hex'], '26')
        self.assertEqual([p['name'] for p in previews[0]['url']['params']], ['rlkey', 'dl', '<masked>'])

    def test_odd_payloads_do_not_raise(self):
        self.assertEqual(m.probe(lambda q: [])['hit_count'], 0)
        self.assertEqual(m.probe(search_of('x', None, {'text': None}))['hit_count'], 0)


if __name__ == '__main__':
    unittest.main()
