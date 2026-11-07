# Solmu icons

The mark is two mirrored speech bubbles with both tails pointing down and
outward. Their intersection is cream (#fff0bf). The original 182-unit gap
is retained, not the closer or circle/oval studies.

`macos/icon-1024.svg` is the canonical mark. macOS keeps its green rounded
square; runtime, small, website and Windows assets keep their circular
backgrounds and use the same mark geometry.

Regenerate with `python3 packaging/generate-icons.py` (CairoSVG and Pillow).
Run asset regressions with `python3 packaging/test_icons.py`.
The macOS bundle script generates ICNS from the committed 1024px PNG on macOS.
No ICNS or app build is needed just to update these source assets.
