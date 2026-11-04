import importlib.util
import json
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('probe_mod', pathlib.Path(__file__).with_name('receipt_probe.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

NOW = 1_799_916_900  # 2027-01-14T08:55:00Z, so ages are deterministic
SECRET = 'SECRET-XYZ-123'


def outgoing(seen=None, status='SUCCESS', delivered=(), minutes=5):
    item = {
        'id': SECRET,
        'chatID': SECRET,
        'senderID': SECRET,
        'text': SECRET,
        'isSender': True,
        'timestamp': '2027-01-14T08:50:00Z',  # 5 minutes before NOW
        'sendStatus': {'status': status, 'deliveredToUsers': list(delivered)},
    }
    if seen is not None:
        item['seen'] = seen
    return item


def get_of(page):
    return lambda path, query: page


class ProbeTests(unittest.TestCase):
    def test_every_delivery_shape_maps_to_the_mappers_state(self):
        cases = [
            (outgoing(seen=True), 'read'),
            (outgoing(seen='2027-01-14T08:52:00Z'), 'read'),
            (outgoing(seen={'a': '2027-01-14T08:52:00Z'}), 'read-by-some'),
            (outgoing(seen={'a': False}), 'sent'),
            (outgoing(seen=False), 'sent'),
            (outgoing(delivered=['x']), 'delivered'),
            (outgoing(), 'sent'),
            (outgoing(status='PENDING'), 'pending'),
            (outgoing(status='FAIL_PERMANENT'), 'failed'),
        ]
        for item, expected in cases:
            self.assertEqual(m.delivery_of(item), expected, item)
        item = outgoing()
        del item['sendStatus']
        self.assertEqual(m.delivery_of(item), 'unknown')

    def test_summary_masks_every_private_value(self):
        page = {'items': [outgoing(seen={SECRET: '2027-01-14T08:52:00Z'})]}
        out = m.probe(get_of(page), 'some-chat', 10, now=NOW)
        self.assertNotIn(SECRET, json.dumps(out))
        message = out['messages'][0]
        self.assertEqual(message['minutes_ago'], 5)
        self.assertEqual(message['seen']['shape'], 'map')
        self.assertEqual(message['seen']['entries'], 1)
        self.assertEqual(message['seen']['entries_with_time'], 1)
        self.assertEqual(message['mapper_shows'], 'read-by-some')

    def test_seen_shapes_are_structural(self):
        self.assertEqual(m.seen_shape(None, True), {'shape': 'absent'})
        self.assertEqual(m.seen_shape(True, True), {'shape': 'bool', 'value': True})
        self.assertEqual(m.seen_shape('2027-01-14T08:52:00Z', True),
                         {'shape': 'string', 'is_timestamp': True})
        self.assertEqual(m.seen_shape('soon', True), {'shape': 'string', 'is_timestamp': False})

    def test_incoming_and_other_chats_messages_are_left_out(self):
        page = {'items': [outgoing(), {'isSender': False, 'timestamp': '2027-01-14T08:54:00Z'}]}
        out = m.probe(get_of(page), 'some-chat', 10, now=NOW)
        self.assertEqual(out['outgoing_in_page'], 1)

    def test_chat_selection_picks_latest_activity_without_printing_ids(self):
        chats = {'items': [
            {'id': 'OLDCHAT', 'lastActivity': '2027-01-01T00:00:00Z'},
            {'id': 'NEWCHAT', 'lastActivity': '2027-01-14T08:00:00Z'},
        ]}
        calls = []

        def get(path, query):
            calls.append(path)
            return chats if path == '/v1/chats' else {'items': [outgoing()]}

        out = m.probe(get, None, 10, now=NOW)
        self.assertIn('/v1/chats/NEWCHAT/messages', calls[-1])
        self.assertNotIn('OLDCHAT', json.dumps(out['messages']))


if __name__ == '__main__':
    unittest.main()
