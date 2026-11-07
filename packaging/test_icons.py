"""Asset regression checks, no native app build needed."""
from pathlib import Path
import struct
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
NS = {'s': 'http://www.w3.org/2000/svg'}


class IconTests(unittest.TestCase):
    def test_mirrored_mark_matches_every_surface(self):
        names = ['packaging/macos/icon-1024.svg', 'packaging/icons/zapfast.svg',
                 'packaging/icons/zapfast-small.svg', 'docs/assets/images/logo.svg']
        for name in names:
            with self.subTest(name=name):
                tree = ET.parse(ROOT / name)
                groups = tree.findall('.//s:g', NS)
                transforms = [g.get('transform') for g in groups]
                self.assertIn('translate(46 140) scale(.74)', transforms)
                self.assertIn('translate(228 140) scale(.74) translate(1024 0) scale(-1 1)', transforms)
                cream = [p for p in tree.findall('.//s:path', NS) if p.get('fill') == '#fff0bf']
                self.assertEqual(len(cream), 1)
                self.assertEqual(cream[0].get('d'), 'M7.9 20A9 9 0 1 0 4 16.1L2 22Z')

    def test_macos_raster_is_1024_square(self):
        png = (ROOT / 'packaging/macos/icon-1024.png').read_bytes()
        self.assertEqual(png[:8], b'\x89PNG\r\n\x1a\n')
        self.assertEqual(struct.unpack('>II', png[16:24]), (1024, 1024))

    def test_windows_has_small_and_large_frames(self):
        ico = (ROOT / 'packaging/windows/solmu.ico').read_bytes()
        reserved, kind, count = struct.unpack('<HHH', ico[:6])
        self.assertEqual((reserved, kind), (0, 1))
        sizes = {ico[6 + index * 16] or 256 for index in range(count)}
        self.assertEqual(sizes, {16, 24, 32, 48, 64, 128, 256})


if __name__ == '__main__':
    unittest.main()
