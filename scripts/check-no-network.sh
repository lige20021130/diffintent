#!/usr/bin/env bash
# spec §10：构造上不可能调网。判定方式 = 闭集白名单（Task 1 复核裁定 MA4，取代原 denylist）。
#
# denylist 的失败模式是"没想到的名字漏过去"——旧关键词表实测放过了
# sqlx / diesel / redis / curl / winhttp / mongodb / libsqlite3-sys。
# 这里反过来：normal 依赖图必须 ⊆ scripts/allowed-normal-deps.txt，
# 出现任何白名单外的包名即失败，并点名它。新增依赖必须经过一次显式加白评审。
set -euo pipefail
cd "$(dirname "$0")/.."

ALLOWED=scripts/allowed-normal-deps.txt
[ -f "$ALLOWED" ] || { echo "FAIL: 白名单文件缺失：$ALLOWED（没有它就没有判定依据）"; exit 1; }

# mktemp 而非写死 /tmp（MB4：并发 CI、非 Windows 上的 /tmp 语义、多用户机器都 avoid 掉）。
TREE=$(mktemp)
FOUND=$(mktemp)
WANT=$(mktemp)
trap 'rm -f "$TREE" "$FOUND" "$WANT"' EXIT

# --workspace：xtask / bench-floor 的 normal 依赖同样进门禁。
# 不带它时 cargo tree 只展开 workspace 根包（default-members 只有根包），xtask 是盲区。
if ! cargo tree -e normal --workspace --offline --prefix none >"$TREE"; then
  echo "FAIL: cargo tree 取不到依赖图。判据缺失时拒绝报绿。"
  exit 1
fi

LC_ALL=C awk 'NF {print $1}' "$TREE" | LC_ALL=C sort -u >"$FOUND"
LC_ALL=C grep -Ev '^[[:space:]]*(#|$)' "$ALLOWED" | tr -d '\r' | LC_ALL=C sort -u >"$WANT" || true

UNKNOWN=$(LC_ALL=C comm -23 "$FOUND" "$WANT")
if [ -n "$UNKNOWN" ]; then
  echo "FAIL: 发现白名单外的 normal 依赖（spec §10 要求构造级禁网）："
  while IFS= read -r pkg; do
    [ -n "$pkg" ] || continue
    printf '  - %s  <- %s\n' "$pkg" "$(grep -m1 -E "^${pkg} v" "$TREE" || echo '（未在图中，异常）')"
  done <<<"$UNKNOWN"
  echo "处置：要么移除该依赖，要么在 $ALLOWED 中显式加白并说明它为何不可能联网。"
  echo "反例提醒：不要因为'它这次没联网'就加白——判定的是它能不能联网。"
  exit 1
fi

N=$(wc -l <"$FOUND" | tr -d '[:space:]')
NODES=$(wc -l <"$TREE" | tr -d '[:space:]')
STALE=$(LC_ALL=C comm -13 "$FOUND" "$WANT")
if [ -n "$STALE" ]; then
  echo "提示：白名单有 $(printf '%s\n' "$STALE" | wc -l | tr -d '[:space:]') 条当前不在图中：$(printf '%s ' $STALE)"
  echo '      （不判失败；依赖被移除后请顺手清理，避免留成「以后再加回来不用评审」的暗门）'
fi

echo "normal deps: ${N} 个唯一包 / ${NODES} 个图节点，全部命中白名单"
echo "OK: 无网络依赖"
