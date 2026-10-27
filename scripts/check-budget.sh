#!/usr/bin/env bash
# spec §12 门禁：release 体积 + 冷启动。
# 冷启动按裁定 MA3 改口径：**边际成本**（被测 exe 减同 workspace 生成的参考空二进制）≤ 15 ms，
# 绝对上界 160 ms 只作 CI 冒烟。原因：本机空 hello-world 就要 88 ms，
# spec §12 的 "< 80 ms" 绝对值在 Windows CreateProcess 下不可达——量到的是操作系统，不是我们。
#
# 复核 Critical 1：原 R8 规定的 `date +%s%N` 在 macOS（BSD date）不支持 %N，
# T0/T1 会带上字面量 "…%N" 或退化成秒，`set -euo pipefail` 下整脚本中止，
# 最坏的一种表现是算出 "cold start 0 ms" —— 一个只会报绿的门禁。
# 现在按可用性择一：bash EPOCHREALTIME(>=5.0，内建，零额外进程) → perl Time::HiRes → python perf_counter。
# 三条都没有就 exit 1：没有量具时拒绝报绿。
set -euo pipefail

[ -f Cargo.toml ] || { echo "FAIL: 找不到 Cargo.toml——请在仓库根目录运行（参考二进制要用 cargo -p bench-floor 构建）"; exit 1; }

EXE="${1:-target/release/diffintent}"
FLOOR="${2:-target/release/bench-floor}"

# Windows 上二进制带 .exe。Git Bash 的 uname 是 MINGW64_NT-*，CI 的 windows-latest bash 同理，
# 所以按文件存在性回退探测，不信 uname 字面值（沿用原报告 D4 的结论）。
[ -f "$EXE" ] || [ ! -f "$EXE.exe" ] || EXE="$EXE.exe"
[ -f "$EXE" ] || { echo "FAIL: 二进制不存在: $EXE（先 cargo build --release --locked）"; exit 1; }

if [ ! -f "$FLOOR" ] && [ ! -f "$FLOOR.exe" ]; then
  echo "参考空二进制缺失，构建：cargo build --release --locked -p bench-floor"
  cargo build --release --locked -p bench-floor
fi
[ -f "$FLOOR" ] || [ ! -f "$FLOOR.exe" ] || FLOOR="$FLOOR.exe"
[ -f "$FLOOR" ] || { echo "FAIL: 参考空二进制不存在: $FLOOR —— 边际成本无从测量，拒绝退化成绝对值判定"; exit 1; }

bytes_of() {
  local raw
  raw=$(stat -c%s "$1" 2>/dev/null || wc -c <"$1")
  printf '%s\n' $(( raw + 0 ))
}

SIZE=$(bytes_of "$EXE")
FLOOR_BYTES=$(bytes_of "$FLOOR")
# MB1c：原先 $((SIZE/1024/1024)) 向下取整会打印 "size: 0 MB (601600 bytes)"，
# 真回归时读不出量级差。改成字节 + 一位小数 MB（两个都打，判定用字节）。
echo "size: $(awk -v s="$SIZE" 'BEGIN{printf "%.1f", s/1048576}') MB (${SIZE} bytes)  预算 < 12 MB"
echo "  参考空二进制 bench-floor: ${FLOOR_BYTES} bytes（差值 $(( SIZE - FLOOR_BYTES )) bytes，信息项）"
[ "$SIZE" -lt $((12 * 1024 * 1024)) ] || { echo "FAIL: 超体积预算（${SIZE} bytes ≥ 12582912 bytes）"; exit 1; }

CLOCK=""
if [ -n "${EPOCHREALTIME:-}" ]; then
  CLOCK=bash
elif command -v perl >/dev/null 2>&1 && perl -MTime::HiRes -e 'exit 0' 2>/dev/null; then
  CLOCK=perl
elif command -v python3 >/dev/null 2>&1; then
  CLOCK=python3
elif command -v python >/dev/null 2>&1 && python -c 'import time; time.perf_counter()' 2>/dev/null; then
  CLOCK=python
fi
[ -n "$CLOCK" ] || { echo "FAIL: 找不到毫秒级计时钟（bash>=5 EPOCHREALTIME / perl Time::HiRes / python perf_counter 全无）。宁可失败，不用秒精度报绿。"; exit 1; }

# 打时间戳到全局 US（微秒）。不用命令替换取 EPOCHREALTIME：cygwin 的 fork 本身就几十微秒到毫秒级，
# 让量具的成本留在测量里会污染绝对值（边际成本相减能抵消，绝对值上界不能靠这个）。
stamp() {
  case "$CLOCK" in
    bash)
      local e s f
      e=$EPOCHREALTIME
      s=${e%%[.,]*}
      f=${e##*[.,]}
      US=$(( s * 1000000 + 10#${f:-0} ))
      ;;
    perl) US=$(perl -MTime::HiRes -e 'printf "%.0f", Time::HiRes::time() * 1000000') ;;
    *)    US=$("$CLOCK" -c 'import time; print(int(time.perf_counter() * 1000000))') ;;
  esac
}

# best-of-5，--version 走 clap 的早退路径，不进流水线。
measure() {
  local exe="$1" best=999999999 a b d i
  for i in 1 2 3 4 5; do
    stamp; a=$US
    "$exe" --version >/dev/null 2>&1
    stamp; b=$US
    d=$(( (b - a) / 1000 ))
    [ "$d" -lt "$best" ] && best=$d
  done
  BEST_MS=$best
}

# 先确认被测二进制真能跑，否则时间数字毫无意义。
"$EXE" --version >/dev/null 2>&1 || { echo "FAIL: $EXE --version 执行失败（非零退出），冷启动数值不可信"; exit 1; }

measure "$FLOOR"; FLOOR_MS=$BEST_MS
measure "$EXE";   EXE_MS=$BEST_MS
DELTA_MS=$(( EXE_MS - FLOOR_MS ))

echo "cold start（best of 5，时钟=${CLOCK}）:"
echo "  参考空二进制 bench-floor : ${FLOOR_MS} ms"
echo "  diffintent              : ${EXE_MS} ms"
echo "  边际成本                : ${DELTA_MS} ms   预算 ≤ 15 ms（裁定 MA3）"
echo "  绝对上界                : ${EXE_MS} ms   预算 < 160 ms（仅 CI 冒烟；spec §12 的 80 ms 目标本机不可达）"

[ "$DELTA_MS" -le 15 ] || { echo "FAIL: 冷启动边际成本 ${DELTA_MS} ms > 15 ms（diffintent ${EXE_MS} ms − 参考 ${FLOOR_MS} ms）"; exit 1; }
[ "$EXE_MS" -lt 160 ] || { echo "FAIL: 冷启动 ${EXE_MS} ms 超 160 ms 绝对上界"; exit 1; }
echo "OK: 预算达标"
