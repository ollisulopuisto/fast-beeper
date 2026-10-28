#!/usr/bin/env bash
# Run portable adapter/model and launcher checks. Not a full GUI/native acceptance.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo test --locked --manifest-path harness/Cargo.toml
uv run --no-project python -m unittest discover -s contrib/beeper-tools -p 'test_*.py' -v
