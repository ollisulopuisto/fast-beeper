"""Structural tests for poll_probe: shapes in, no content out."""
import unittest

import poll_probe


class UndocumentedKeysTest(unittest.TestCase):
    def test_documented_payload_has_no_extra_keys(self):
        item = {'id': 'm1', 'text': 'hello', 'type': 'TEXT', 'seen': True}
        self.assertEqual(poll_probe.undocumented_keys(item), [])

    def test_unknown_fields_are_named_without_values(self):
        item = {'id': 'm1', 'type': 'POLL', 'poll': {'question': 'secret'},
                'future': 1}
        self.assertEqual(poll_probe.undocumented_keys(item), ['future', 'poll'])


class ShapeTest(unittest.TestCase):
    def test_histogram_counts_types_and_absence(self):
        items = [{'type': 'TEXT'}, {'type': 'TEXT'}, {'type': 'POLL'}, {}]
        self.assertEqual(poll_probe.type_histogram(items),
                         {'TEXT': 2, 'POLL': 1, 'absent': 1})

    def test_poll_shape_masks_strings_and_counts_options(self):
        item = {'type': 'POLL',
                'poll': {'question': 'what should stay hidden',
                         'options': ['a', 'b', 'c'], 'votes': {'u1': [0]}}}
        shape = poll_probe.poll_shape(item)
        self.assertEqual(shape['type'], 'POLL')
        self.assertEqual(shape['undocumented_keys'], ['poll'])
        markers = shape['poll_markers']['poll']['keys']
        self.assertEqual(markers, ['options', 'question', 'votes'])
        rendered = str(shape)
        self.assertNotIn('what should stay hidden', rendered)
        self.assertNotIn('u1', rendered)

    def test_text_is_never_treated_as_a_poll(self):
        item = {'type': 'TEXT', 'text': 'Poll: lunch?'}
        self.assertFalse(poll_probe.looks_pollish(item))


class CapabilityTest(unittest.TestCase):
    def test_levels_group_by_network(self):
        chats = [
            {'network': 'WhatsApp', 'capabilities': {'poll': 2}},
            {'network': 'WhatsApp', 'capabilities': {'poll': -2}},
            {'network': 'Signal'},
        ]
        summary = poll_probe.capability_summary(chats)
        by_key = {(row['network'], str(row['poll_level'])): row['chats']
                  for row in summary}
        self.assertEqual(by_key[('WhatsApp', '2')], 1)
        self.assertEqual(by_key[('WhatsApp', '-2')], 1)
        self.assertEqual(by_key[('Signal', 'absent')], 1)


class ProbeTest(unittest.TestCase):
    def test_probe_reports_structure_only(self):
        def fake_get(path, query):
            if path == '/v1/chats':
                return {'items': [{'id': 'chat1', 'network': 'WhatsApp',
                                   'lastActivity': '2026-11-04T08:00:00Z',
                                   'capabilities': {'poll': 2}}]}
            return {'items': [
                {'id': 'm1', 'type': 'TEXT', 'text': 'hidden',
                 'timestamp': '2026-11-04T08:00:00Z'},
                {'id': 'm2', 'type': 'POLL', 'poll': {'question': 'hidden'},
                 'timestamp': '2026-11-04T08:01:00Z'},
            ]}
        report = poll_probe.probe(fake_get, None, 50)
        self.assertTrue(report['read_only'])
        self.assertEqual(report['message_types'], {'TEXT': 1, 'POLL': 1})
        self.assertEqual(report['poll_like_messages'], 1)
        self.assertEqual(report['poll_like_shapes'][0]['undocumented_keys'], ['poll'])
        self.assertNotIn('hidden', str(report))
        self.assertNotIn('m2', str(report))


if __name__ == '__main__':
    unittest.main()
