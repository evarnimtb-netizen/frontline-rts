#!/usr/bin/env bash
# tools/run.sh LOG CMD...   start CMD in the background (output to LOG), wait
#                           up to 200 s, print one status line and LOG's tail.
# tools/run.sh --wait LOG   wait up to 200 s more for the same job.
# The exit code is kept in LOG.exit; the pid in LOG.pid. No waiter outlives
# this call: the wait is a poll loop in this process.
LIMIT=${RUN_WAIT_S:-200}
if [ "${1:-}" = "--wait" ]; then
  LOG=${2:?usage: tools/run.sh --wait LOG}
  [ -f "$LOG.pid" ] || [ -f "$LOG.exit" ] || { echo "no job for $LOG" >&2; exit 2; }
else
  [ $# -ge 2 ] || { echo "usage: tools/run.sh LOG CMD... | tools/run.sh --wait LOG" >&2; exit 2; }
  LOG=$1; shift
  rm -f "$LOG.exit"
  # The job's own subshell writes the exit code when CMD ends.
  nohup bash -c 'shift_log=$1; shift; "$@" >"$shift_log" 2>&1; echo $? >"$shift_log.exit"' _ "$LOG" "$@" >/dev/null 2>&1 &
  echo $! >"$LOG.pid"
  disown 2>/dev/null
fi
s=0
while [ ! -f "$LOG.exit" ] && [ "$s" -lt "$LIMIT" ]; do sleep 1; s=$((s+1)); done
if [ -f "$LOG.exit" ]; then
  echo "done, exit code $(cat "$LOG.exit") after about ${s} s of waiting (log $LOG)"
else
  echo "still running (pid $(cat "$LOG.pid" 2>/dev/null)); call: tools/run.sh --wait $LOG"
fi
tail -n 5 "$LOG" 2>/dev/null
