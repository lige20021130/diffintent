#!/usr/bin/env bash
# spec §10：构造上不可能调网。任何 HTTP/TLS/加密/DB 客户端进入 normal 依赖即失败。
set -euo pipefail
cargo tree -e normal --offline > /tmp/diffintent-normal-deps.txt
if grep -nE '(^|[^a-z0-9_])(reqwest|hyper|isahc|ureq|surf|attohttpc|minreq|rustls|native-tls|openssl|schannel|awc|postgres|rusqlite|mysql|libsqlite|awc)([^a-z0-9_]|$)' /tmp/diffintent-normal-deps.txt; then
  echo "FAIL: 发现网络/加密/数据库依赖（见上行）。spec §10 禁止。"
  echo "若 hyper 是 tree-sitter 相关 crate 的非正常出现，应移除该 crate 而非加白名单。"
  exit 1
fi
N=$(grep -cE '^[^ ]' /tmp/diffintent-normal-deps.txt || true)
echo "normal deps: ${N} (预算 ≤ 38 个 crate 节点；主 crate 直接依赖 ≤ 8)"
echo "OK: 无网络依赖"
