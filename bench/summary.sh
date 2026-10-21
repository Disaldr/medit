#!/bin/bash
B=$(cd "$(dirname "$0")" && pwd); W=${1:-/tmp/medit-bench}
printf "%-8s %5s %6s %6s %9s %9s %7s %s\n" run turns sec cost in_tok out_tok fails tools
for f in $W/*.jsonl; do
  n=$(basename $f .jsonl)
  r=$(grep '"type":"result"' $f | tail -1)
  tools=$(jq -r 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use") | .name' $f 2>/dev/null | sort | uniq -c | awk '{printf "%s×%s ", $2, $1}')
  echo "$r" | jq -r --arg n "$n" --arg fails "$(tail -1 $W/$n.check)" --arg tools "$tools" \
    '[$n, .num_turns, (.duration_ms/1000|floor), (.total_cost_usd*1000|round/1000),
      (.usage.input_tokens + .usage.cache_read_input_tokens + .usage.cache_creation_input_tokens),
      .usage.output_tokens, $fails, $tools, (.permission_denials|length)] | @tsv' |
    awk -F'\t' '{printf "%-8s %5s %6s %6s %9s %9s %7s %s denials=%s\n",$1,$2,$3,$4,$5,$6,$7,$8,$9}'
done
