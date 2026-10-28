#!/usr/bin/env bash
# Run portable adapter/model and launcher checks. Not a full GUI/native acceptance.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
uv run --no-project python contrib/beeper-tools/check_adapter.py
uv run --no-project python -m unittest discover -s contrib/beeper-tools -p 'test_*.py' -v
