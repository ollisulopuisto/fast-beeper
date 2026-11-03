import importlib.util
import json
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('probe_mod', pathlib.Path(__file__).with_name('raw_text_probe.py'))
probe_mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe_mod)


class ProbeTests(unittest.TestCase):
    def test_window_keeps_entity_bytes_and_nothing_else(self):
        secret = 'private words before ' + 'x' * 40
        text = secret + ' https://x.test/f?rlkey=k&amp;dl=0 more private text after'
        out = probe_mod.probe(lambda q: {'items': [{'text': text, 'type': 'TEXT', 'chatID': '!room', 'senderID': '@a'}]}, 'dl=0')
        hit = out['hits'][0]
        self.assertIn('&amp;dl=0', json.loads(hit['escaped_text']))
        self.assertIn('26 61 6d 70 3b', hit['utf8_hex'])
        self.assertNotIn('private words', json.dumps(out))
        self.assertNotIn('more private', json.dumps(out))
        self.assertNotIn('!room', json.dumps(out))
        self.assertFalse(hit['text_contains_angle_bracket'])

    def test_bare_ampersand_and_hit_limit(self):
        items = [{'text': 'a https://x.test/?rlkey=k&dl=0', 'type': 'TEXT'} for _ in range(5)]
        out = probe_mod.probe(lambda q: {'items': items}, 'dl=0')
        self.assertEqual(out['hit_count'], 3)
        self.assertIn('26 64 6c', out['hits'][0]['utf8_hex'])

    def test_no_match_and_odd_payloads(self):
        self.assertEqual(probe_mod.probe(lambda q: {'items': [{'text': 'nothing'}]}, 'dl=0')['hit_count'], 0)
        self.assertEqual(probe_mod.probe(lambda q: [], 'dl=0')['hit_count'], 0)
        self.assertEqual(probe_mod.probe(lambda q: {'items': ['x', None]}, 'dl=0')['hit_count'], 0)


if __name__ == '__main__':
    unittest.main()
