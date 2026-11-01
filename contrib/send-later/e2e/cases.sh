#!/bin/bash
# Controlled send-later tests: the real zapfast-scheduler daemon against a
# stub Beeper Server API. Run via run-e2e.sh, which builds the daemon from
# this repo's sources first. Ports/paths overridable for parallel runs.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
BIN=${SL_E2E_BIN:-$HERE/../../target/release/zapfast-scheduler}
WORK=${SL_E2E_WORK:-/tmp/sltest}
STUB_PORT=${SL_E2E_STUB_PORT:-23901}
DAEMON_PORT=${SL_E2E_DAEMON_PORT:-23902}
mkdir -p "$WORK"
Q=$WORK/queue.json
DAEMON=127.0.0.1:$DAEMON_PORT
AUTH=(-H "Authorization: Bearer ctl" -H "Content-Type: application/json")
PASS=0; FAIL=0
ok() { if [ "$1" = "$2" ]; then PASS=$((PASS+1)); echo "PASS: $3"; else FAIL=$((FAIL+1)); echo "FAIL: $3 (want [$2] got [$1])"; fi; }

start_daemon() {
  BEEPER_BASE_URL=http://127.0.0.1:$STUB_PORT BEEPER_ACCESS_TOKEN=x ZAPFAST_CONTROL_TOKEN=ctl \
  ZAPFAST_QUEUE_PATH=$Q ZAPFAST_LISTEN=$DAEMON ZAPFAST_TICK_SECS=1 \
  "$BIN" >> $WORK/daemon.log 2>&1 &
  echo $! > $WORK/daemon.pid
  sleep 0.7
}
stop_daemon() { kill $(cat $WORK/daemon.pid) 2>/dev/null; sleep 0.3; }
list_states() { curl -s "${AUTH[@]}" http://$DAEMON/v1/scheduled | python3 -c "import json,sys; print(','.join(i['state'] for i in json.load(sys.stdin)))" 2>/dev/null; }
add() { curl -s "${AUTH[@]}" -d "{\"account_id\":\"acc-1\",\"chat_id\":\"chat-1\",\"text\":\"$1\",\"due_at\":$2}" http://$DAEMON/v1/scheduled | python3 -c "import json,sys; print(json.load(sys.stdin).get('id','ERR'))" 2>/dev/null; }
sends() { python3 -c "
import json,urllib.request
d=json.load(urllib.request.urlopen('http://127.0.0.1:$STUB_PORT/v1/messages/search?query=x'))
print(len(d['items']))"; }
sent_texts() { python3 -c "
import json,urllib.request
d=json.load(urllib.request.urlopen('http://127.0.0.1:$STUB_PORT/v1/messages/search?query=x'))
print('|'.join(i['text'] for i in d['items']))"; }
state_of() { curl -s "${AUTH[@]}" http://$DAEMON/v1/scheduled | python3 -c "import json,sys; d=json.load(sys.stdin); m=[x for x in d if x['id']=='$1']; print(m[0]['state'] if m else 'MISSING', (m[0].get('error') or '')[:12] if m else '')" 2>/dev/null; }

pkill -f 'zapfast-scheduler' 2>/dev/null; sleep 0.5
rm -f $Q $WORK/daemon.log $WORK/mode
SL_STUB_PORT=$STUB_PORT python3 "$HERE/stub.py" & echo $! > $WORK/stub.pid
sleep 0.5
start_daemon

# T0: socket hardening
code=$(curl -s -o /dev/null -w '%{http_code}' -H "Origin: http://evil.example" "${AUTH[@]}" -d '{}' http://$DAEMON/v1/scheduled)
ok "$code" "403" "T0a POST with Origin header refused"
code=$(curl -s -o /dev/null -w '%{http_code}' -H "Content-Type: application/json" -d '{}' http://$DAEMON/v1/scheduled)
ok "$code" "403" "T0b missing control token refused"
code=$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer ctl" -H "Content-Type: text/plain" -d 'x' http://$DAEMON/v1/scheduled)
ok "$code" "403" "T0c non-JSON POST refused"

# T1: schedule and deliver
NOW=$(date +%s)
id1=$(add "controlled test one" $((NOW+2)))
ok "$([ ${#id1} -gt 5 ] && echo yes)" "yes" "T1a add accepted (id $id1)"
sleep 4
ok "$(sends)" "1" "T1b due message delivered once"
ok "$(sent_texts)" "controlled test one" "T1c exact words delivered"
ok "$(state_of $id1)" "Sent " "T1d state Sent"
curl -s "${AUTH[@]}" http://$DAEMON/v1/scheduled | python3 -c "import json,sys; print(json.load(sys.stdin)[0].get('sent_message_id'))" | grep -q stub-1 && ok yes yes "T1e sent_message_id recorded" || ok no yes "T1e sent_message_id recorded"

# T2: cancel before due
id2=$(add "never send this" $((NOW+60)))
curl -s "${AUTH[@]}" -d '{}' http://$DAEMON/v1/scheduled/$id2/cancel > /dev/null
sleep 3
ok "$(sends)" "1" "T2a cancelled message never sent"
ok "$(state_of $id2)" "Cancelled " "T2b state Cancelled"

# T3: edit text and due time
id3=$(add "original words" $((NOW+90)))
curl -s "${AUTH[@]}" -d "{\"text\":\"edited words\",\"due_at\":$(($(date +%s)+1))}" http://$DAEMON/v1/scheduled/$id3/edit > /dev/null
sleep 3
ok "$(sent_texts | grep -c 'edited words')" "1" "T3a edited words delivered"
ok "$(sent_texts | grep -c 'original words')" "0" "T3b original words never sent"

# T4: definite rejection, no auto-retry
echo reject > $WORK/mode
NOW=$(date +%s)
id4=$(add "rejection test" $((NOW+1)))
sleep 3
ok "$(state_of $id4 | grep -c '^Failed .*500')" "1" "T4a rejected send marked Failed with 500 reason"
sleep 2
ok "$(sends)" "2" "T4b no automatic retry after rejection"
echo normal > $WORK/mode

# T5: kill -9 mid-send -> Uncertain on restart, no auto-resend, no reconcile endpoint
echo hang > $WORK/mode
NOW=$(date +%s)
id5=$(add "crash test" $((NOW+1)))
sleep 2
kill -9 $(cat $WORK/daemon.pid); sleep 0.5
qstate=$(python3 -c "
import json; d=json.load(open('$Q'))
items = d.get('items', d) if isinstance(d, dict) else d
print([i['state'] for i in items if i.get('id')=='$id5'][0])")
ok "$qstate" "Sending" "T5a crash leaves item claimed on disk"
: > $WORK/daemon.log
start_daemon
grep -q 'crashed mid-send, now Uncertain' $WORK/daemon.log && r=yes || r=no
ok "$r" "yes" "T5b restart recovers item as Uncertain"
sleep 3
ok "$(sends)" "2" "T5c no automatic resend of Uncertain"
rb=$(curl -s "${AUTH[@]}" -d '{}' http://$DAEMON/v1/scheduled/$id5/reconcile)
ok "$rb" '{"reconcile":"not_found"}' "T5d reconcile endpoint: not_found when the send never landed"
ok "$(state_of $id5)" "Uncertain " "T5e reconcile keeps Uncertain on not_found"
code=$(curl -s -o /dev/null -w '%{http_code}' "${AUTH[@]}" -d '{}' http://$DAEMON/v1/scheduled/$id5/retry)
ok "$code" "409" "T5f retry of Uncertain without confirm_duplicate refused"
ok "$(sends)" "2" "T5g refused retry did not send"
echo normal > $WORK/mode
curl -s "${AUTH[@]}" -d '{"confirm_duplicate":true}' http://$DAEMON/v1/scheduled/$id5/retry > /dev/null
sleep 2
ok "$(sends)" "3" "T5h confirmed retry resends (duplicate when first send had landed)"
ok "$(state_of $id5)" "Sent " "T5i confirmed retry lands as Sent"
code=$(curl -s -o /dev/null -w '%{http_code}' "${AUTH[@]}" -d '{}' http://$DAEMON/v1/scheduled/$id5/reconcile)
ok "$code" "409" "T5j reconcile refuses a non-Uncertain item"

# T6: clean restart still fires on time
NOW=$(date +%s)
id6=$(add "restart durability" $((NOW+2)))
stop_daemon
start_daemon
sleep 3
ok "$(sent_texts | grep -c 'restart durability')" "1" "T6 queued item survives clean restart and fires"

stop_daemon; kill $(cat $WORK/stub.pid) 2>/dev/null
wait 2>/dev/null
echo "== $PASS passed, $FAIL failed =="
