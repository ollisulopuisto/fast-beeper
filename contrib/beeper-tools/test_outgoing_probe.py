"""Tests for outgoing_probe's flag logic, with synthetic payloads only."""
import unittest

from outgoing_probe import probe, summarize, verdict


def message(**overrides):
    base = {
        'id': 'm',
        'isSender': False,
        'isHidden': False,
        'isDeleted': False,
        'type': 'TEXT',
        'text': 'x',
        'timestamp': '2026-11-04T14:00:00Z',
    }
    base.update(overrides)
    return base


def page_get(items):
    def get(path, query):
        if path == '/v1/chats':
            return {'items': [{'id': 'c', 'lastActivity': '2026-11-04T14:05:00Z'}]}
        return {'items': items}
    return get


NOW = 1_793_801_400  # 2026-11-04T14:10:00Z


class VerdictTest(unittest.TestCase):
    def test_outgoing_absent(self):
        self.assertEqual(verdict([message(), message()]), 'outgoing-absent-from-page')

    def test_outgoing_clean(self):
        self.assertEqual(
            verdict([message(), message(isSender=True)]),
            'outgoing-clean-in-page')

    def test_outgoing_hidden(self):
        self.assertEqual(
            verdict([message(isSender=True, isHidden=True)]),
            'outgoing-suppressed-by-isHidden')

    def test_outgoing_deleted(self):
        self.assertEqual(
            verdict([message(isSender=True, isDeleted=True)]),
            'outgoing-suppressed-by-isDeleted')

    def test_outgoing_hidden_and_deleted(self):
        self.assertEqual(
            verdict([message(isSender=True, isHidden=True, isDeleted=True)]),
            'outgoing-suppressed-by-isDeleted-and-isHidden')


class ProbeTest(unittest.TestCase):
    def test_counts_and_order(self):
        items = [
            message(timestamp='2026-11-04T14:01:00Z'),
            message(isSender=True, timestamp='2026-11-04T14:03:00Z', isHidden=True),
            message(isSender=True, timestamp='2026-11-04T14:02:00Z'),
        ]
        result = probe(page_get(items), None, 20, now=NOW)
        self.assertEqual(result['page_size'], 3)
        self.assertEqual(result['outgoing_in_page'], 2)
        self.assertEqual(result['outgoing_app_would_show'], 1)
        self.assertEqual(result['verdict'], 'outgoing-suppressed-by-isHidden')
        # Newest first.
        self.assertEqual([m['minutes_ago'] for m in result['messages']], [7, 8, 9])
        self.assertEqual(result['messages'][0]['direction'], 'outgoing')
        self.assertFalse(result['messages'][0]['app_would_show'])
        self.assertTrue(result['messages'][1]['app_would_show'])

    def test_unknown_sender_direction(self):
        entry = summarize(message(isSender=None), NOW)
        self.assertEqual(entry['direction'], 'unknown')

    def test_empty_page(self):
        result = probe(page_get([]), None, 20, now=NOW)
        self.assertEqual(result['page_size'], 0)
        self.assertEqual(result['verdict'], 'outgoing-absent-from-page')

    def test_no_chats(self):
        result = probe(lambda p, q: {'items': []}, None, 20, now=NOW)
        self.assertEqual(result.get('error'), 'no chats')

    def test_no_text_or_ids_leak(self):
        result = probe(page_get([
            message(isSender=True, text='secret body')]), None, 20, now=NOW)
        rendered = str(result)
        self.assertNotIn('secret body', rendered)
        self.assertEqual(result['messages'][0]['text_length'], 11)


if __name__ == '__main__':
    unittest.main()
