(
set -eu
umask 077
TOKEN_FILE="$HOME/Library/Application Support/zapfast/scheduler-token"
[ -f "$TOKEN_FILE" ] || exit 1
# mktemp needs an explicit template: the macOS BSD mktemp has no default
# (bare mktemp is a GNU-ism and exits 1 there, failing every health check).
header=$(mktemp "${TMPDIR:-/tmp}/solmu-health.XXXXXX")
trap 'rm -f "$header" "$header.response"' EXIT HUP INT TERM
printf 'Authorization: Bearer %s\n' "$(cat "$TOKEN_FILE")" > "$header"
curl --noproxy '*' -sf -m 5 --header "@$header" http://127.0.0.1:23376/v1/health > "$header.response"
grep -q '"ok":true' "$header.response"
)
