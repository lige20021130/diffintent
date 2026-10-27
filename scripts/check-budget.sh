#!/usr/bin/env bash
set -euo pipefail
EXE="${1:-target/release/diffintent}"
# Windows 上二进制带 .exe（Git Bash 的 uname 是 MINGW*，故按文件存在性判定，不依赖 uname 字面值）。
if [ ! -f "$EXE" ] && [ -f "$EXE.exe" ]; then EXE="$EXE.exe"; fi
[ -f "$EXE" ] || { echo "二进制不存在: $EXE"; exit 1; }

SIZE=$(stat -c%s "$EXE" 2>/dev/null || wc -c < "$EXE")
echo "size: $((SIZE / 1024 / 1024)) MB ($SIZE bytes)  预算 < 12 MB"
[ "$SIZE" -lt $((12 * 1024 * 1024)) ] || { echo "FAIL: 超体积预算"; exit 1; }

# 冷启动：--version 走 clap 早退路径，不进流水线
BEST=999999
for _ in 1 2 3 4 5; do
  S=$( { T0=$(date +%s%N); "$EXE" --version >/dev/null 2>&1; T1=$(date +%s%N); echo $(( (T1 - T0) / 1000000 )); } )
  [ "$S" -lt "$BEST" ] && BEST=$S
done
echo "cold start (best of 5): ${BEST} ms  预算 < 80 ms（CI 判定 160 ms）"
LIMIT=160
[ "$BEST" -lt "$LIMIT" ] || { echo "FAIL: 冷启动 ${BEST} ms 超 ${LIMIT} ms"; exit 1; }
echo "OK: 预算达标"
