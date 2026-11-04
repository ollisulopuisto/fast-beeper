"""Tests for outgoing_probe's flag and pagination logic, synthetic payloads."""
import unittest

from outgoing_probe import fetch_pages, probe, summarize, verdict


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


NOW = 1_793_801_400  # 2026-11-04T14:10:00Z


def pages_get(pages_by_cursor, chats=None):
    """pages_by_cursor: {cursor-or-None: page dict}; chats for selection."""
    def get(path, query):
        if path == '/v1/chats':
            return {'items': chats or [{'id': 'c', 'title': 'T',
                                        'lastActivity': '2026-11-04T14:05:00Z'}]}
        return pages_by_cursor.get(query.get('cursor'), {'items': []})
    return get


class VerdictTest(unittest.TestCase):
    def test_outgoing_absent(self):
        self.assertEqual(verdict([message()], 3, False),
                         'outgoing-absent-from-3-pages-partial-history')

    def test_outgoing_absent_complete(self):
        self.assertEqual(verdict([], 3, True), 'outgoing-absent-from-3-pages')

    def test_outgoing_clean(self):
        self.assertEqual(verdict([message(), message(isSender=True)], 1, True),
                         'outgoing-clean-in-1-pages')

    def test_outgoing_hidden(self):
        self.assertEqual(verdict([message(isSender=True, isHidden=True)], 1, True),
                         'outgoing-suppressed-by-isHidden')

    def test_outgoing_hidden_and_deleted(self):
        self.assertEqual(
            verdict([message(isSender=True, isHidden=True, isDeleted=True)], 2, False),
            'outgoing-suppressed-by-isDeleted-and-isHidden')


class FetchPagesTest(unittest.TestCase):
    def test_walks_older_pages_until_complete(self):
        get = pages_get({
            None: {'items': [message(id='a')], 'hasMore': True, 'oldestCursor': 'c1'},
            'c1': {'items': [message(id='b')], 'hasMore': True, 'oldestCursor': 'c2'},
            'c2': {'items': [message(id='c')], 'hasMore': False},
        })
        items, pages, complete, stalled = fetch_pages(get, 'chat', 10)
        self.assertEqual((len(items), pages, complete, stalled), (3, 3, True, None))

    def test_page_cap_marks_partial(self):
        get = pages_get({
            None: {'items': [message()], 'hasMore': True, 'oldestCursor': 'c1'},
            'c1': {'items': [message()], 'hasMore': True, 'oldestCursor': 'c2'},
        })
        items, pages, complete, stalled = fetch_pages(get, 'chat', 1)
        self.assertEqual((pages, complete, stalled), (1, False, None))

    def test_missing_cursor_is_stalled(self):
        get = pages_get({None: {'items': [message()], 'hasMore': True}})
        _, _, complete, stalled = fetch_pages(get, 'chat', 3)
        self.assertEqual((complete, stalled), (False, 'missing-cursor'))

    def test_repeated_cursor_is_stalled(self):
        get = pages_get({
            None: {'items': [message()], 'hasMore': True, 'oldestCursor': 'c1'},
            'c1': {'items': [message()], 'hasMore': True, 'oldestCursor': 'c1'},
        })
        _, pages, complete, stalled = fetch_pages(get, 'chat', 5)
        self.assertEqual((pages, complete, stalled), (2, False, 'repeated-cursor'))


class ProbeTest(unittest.TestCase):
    def test_counts_order_and_pick(self):
        get = pages_get({
            None: {'items': [
                message(timestamp='2026-11-04T14:01:00Z'),
                message(isSender=True, timestamp='2026-11-04T14:03:00Z',
                        isHidden=True),
                message(isSender=True, timestamp='2026-11-04T14:02:00Z'),
            ], 'hasMore': False},
        })
        result = probe(get, None, 3, 30, pick=lambda: '1', now=NOW)
        self.assertEqual(result['chat_selected'], 'interactive-pick')
        self.assertEqual(result['outgoing_in_pages'], 2)
        self.assertEqual(result['outgoing_app_would_show'], 1)
        self.assertEqual(result['verdict'], 'outgoing-suppressed-by-isHidden')
        self.assertTrue(result['history_complete'])
        self.assertEqual([m['minutes_ago'] for m in result['messages']], [7, 8, 9])
        self.assertEqual(result['messages'][0]['direction'], 'outgoing')

    def test_unknown_sender_direction(self):
        entry = summarize(message(isSender=None), NOW)
        self.assertEqual(entry['direction'], 'unknown')
        get = pages_get({None: {'items': [message(isSender=None)], 'hasMore': False}})
        result = probe(get, 'given-chat', 1, 30, now=NOW)
        self.assertEqual(result['unknown_direction'], 1)

    def test_no_chats(self):
        result = probe(lambda p, q: {'items': []}, None, 3, 30, now=NOW)
        self.assertEqual(result.get('error'), 'no chats')

    def test_no_text_or_ids_leak(self):
        get = pages_get({None: {'items': [
            message(isSender=True, text='secret body')], 'hasMore': False}})
        result = probe(get, 'given-chat', 1, 30, now=NOW)
        self.assertNotIn('secret body', str(result))
        self.assertEqual(result['messages'][0]['text_length'], 11)


if __name__ == '__main__':
    unittest.main()
