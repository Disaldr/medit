#!/bin/bash
# check.sh <dir> — печатает число проваленных проверок
d=$1/src; f=0
fail(){ echo "  FAIL: $1"; f=$((f+1)); }
grep -q fetchUser $d/*.ts && fail "остался fetchUser"
[ "$(grep -o loadUser $d/*.ts | wc -l)" -eq 8 ] || fail "loadUser != 8"
grep -q "timeout: 10000," $d/config.ts || fail timeout
grep -q "retries: 5," $d/config.ts || fail retries
grep -q "pageSize: 50," $d/config.ts || fail pageSize
[ -f $d/logger.ts ] || fail "нет logger.ts"
for x in api users orders utils main; do
  grep -q "console.log" $d/$x.ts && fail "console.log в $x"
  grep -q "logger.info" $d/$x.ts || fail "нет logger.info в $x"
  grep -Eq "import \{ logger \} from ['\"]\./logger['\"]" $d/$x.ts || fail "нет импорта в $x"
done
[ "$(grep -o 'logger.info' $d/{api,users,orders,utils,main}.ts | wc -l)" -eq 13 ] || fail "logger.info != 13"
echo "fails=$f"
