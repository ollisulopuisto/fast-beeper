#!/usr/bin/env python3
"""Regenerate icon assets: python -m pip install cairosvg pillow first."""
from pathlib import Path
import io
import xml.etree.ElementTree as ET

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'packaging/macos/icon-1024.svg'
SVG = '{http://www.w3.org/2000/svg}'


def main():
    source = SOURCE.read_text()
    mark_start = source.index('<defs><clipPath')
    mark = source[mark_start:source.rindex('</svg>')]
    # Preserve each surface's existing circular background. The macOS tile
    # remains the canonical mark geometry, scaled from 1024 to 128 units.
    for name in ['packaging/icons/zapfast.svg',
                 'packaging/icons/zapfast-small.svg',
                 'docs/assets/images/logo.svg']:
        path = ROOT / name
        old = path.read_text()
        start = old.index('<g transform="scale(.125)">')
        path.write_text(old[:start] + '<g transform="scale(.125)">' + mark + '</g></svg>\n')
    png = cairosvg.svg2png(bytestring=source.encode(), output_width=1024, output_height=1024)
    (ROOT / 'packaging/macos/icon-1024.png').write_bytes(png)
    runtime = (ROOT / 'packaging/icons/zapfast.svg').read_bytes()
    png = cairosvg.svg2png(bytestring=runtime, output_width=1024, output_height=1024)
    image = Image.open(io.BytesIO(png)).convert('RGBA')
    image.save(ROOT / 'packaging/windows/solmu.ico',
               sizes=[(size, size) for size in [16, 24, 32, 48, 64, 128, 256]])
    for name in ['packaging/macos/icon-1024.svg', 'packaging/icons/zapfast.svg',
                 'packaging/icons/zapfast-small.svg', 'docs/assets/images/logo.svg']:
        tree = ET.parse(ROOT / name)
        assert tree.getroot().tag == SVG + 'svg'
    print('Generated macOS PNG, runtime/web SVGs and Windows ICO.')


if __name__ == '__main__':
    main()
