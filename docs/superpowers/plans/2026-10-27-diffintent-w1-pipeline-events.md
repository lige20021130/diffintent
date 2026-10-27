# diffintent W1（流水线与结构事件）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 `diffintent` Rust crate 的前三段流水线（`patch_parse` → `evidence_resolve` → `event_extract`），产出携带证据层级的规范序 `seir::Event` 序列，并让确定性、体积、冷启动、禁网四条门禁从第一天起就在 CI 上生效。

**Architecture:** 七段线性流水线，段间只传 `seir::Event` 与（W2 起的）`Block`。本计划交付第 1–3 段。检测器是统一签名 `fn(&DetectorCtx, &mut EventSink)` 的函数，按固定顺序存在 `static DETECTORS` 数组里——不用 trait object，不让注册顺序参与结果。文件到 `FileId` 的映射按**路径字典序**赋值而非按遇到顺序，这样输入顺序打乱后输出逐字节不变。

**Tech Stack:** Rust、tree-sitter 0.2x + `tree-sitter-javascript` / `tree-sitter-typescript` / `tree-sitter-python`、clap 4、serde + serde_json、thiserror、insta（快照）、proptest（性质测试）。

**Spec:** [docs/superpowers/specs/2026-10-27-diffintent-design.md](../specs/2026-10-27-diffintent-design.md) — 本计划实现其 §4、§5、§6，并为 §7–§11 铺设输入。**执行者必须同时读 spec**：计划论证取自 spec，两者冲突时以 spec 为准并停下来问。

## Global Constraints

每个任务都隐含遵守本节；数值逐字取自 spec。

- 名称：crate / 二进制 / npm / PyPI 一律 `diffintent`（不是 `diff-intent`）。契约 `schema` 字符串为 `"diffintent/1"`。
- 输入 flag 是 `--request`，**不是** `--intent`（`intent` 是输出字段名）。
- **禁止任何 HTTP / TLS / 加密 / 数据库客户端 crate 进入 normal 依赖。** 门禁在 Task 1 建立，此后每个任务都必须让它保持绿色。不实现 `--llm-fallback`。
- 确定性：同一输入跑两次、以及打乱输入文件顺序，输出必须**逐字节相同**。禁止在排序/序列化路径依赖 HashMap 迭代序。
- 证据层级只有三档：`full_ast` | `hunk_only` | `text_fallback`。每个 `Event` 必须携带 tier，**任何渲染都不得隐藏 tier**。
- 结构事件 kind 固定 14 个：`signature` `guard` `control_flow` `error_handling` `expression` `formatting` `comment` `binding` `call` `dependency` `module` `file` `test_case` `config_key`。**加新 kind 必须先改 spec**，不得在实现里就地发明。
- 事件方向三值：`added` | `removed` | `modified`。序列化形式恒为 `kind:direction`（如 `error_handling:removed`）。
- AST 支持范围：TS / TSX / JS / JSX / MJS / CJS / Python。**其余语言必须降级并标 `grammar_unsupported`，绝不出具看似 confident 的事件集。**
- 依赖预算：normal 依赖 crate ≤ 8 个；dev / build 依赖不计。
- 体积 < 12 MB；`--version` 冷启动 < 80 ms（CI 判定用 160 ms，实际值必须打印）；50 文件 / 2000 行 diff 处理 < 300 ms（CI 判定用 600 ms，实际值必须打印）。
- `unsafe_code` = deny；`missing_docs` 对 `pub` 项 = deny；clippy `-D warnings`；`cargo fmt --check` 必须过。
- 每个任务结束提交一次，conventional commits（`feat:` / `test:` / `ci:` / `fix:`）。
- **本计划不建立任何 `diffintent/1` 对外承诺。** 契约快照测试属 W3。W1 唯一的对外面是 `--dump-events`（spec §2 明确标注非契约）。

---

## 文件结构

先画定边界，再定任务。每个文件一个职责，一起变的放一起。

```
Cargo.toml
build.rs                                pin grammar headers; NO network
.cargo/config.toml                      CC env passthrough on Windows MSVC
src/main.rs                             CLI surface: --file/--repo/--json/--dump-events/--version
src/lib.rs                              库入口 + Pipeline::run()（只到 event_extract）
src/seir/mod.rs                         EventKind(14) Direction Event EvidenceTier ScopeRef FileId + 规范序
src/seir/tag.rs                         "kind:direction" 字符串形式
src/seir/table.rs                       FileTable：路径 → FileId（按路径字典序赋值）
src/contract/mod.rs                     仅 TierReason / DegradationRecord（W1 不建完整契约）
src/patch/mod.rs                        PatchError + parse() 入口
src/patch/model.rs                      FileDiff FileChange Hunk DiffLine
src/patch/parse.rs                        unified diff → Vec<FileDiff>
src/patch/rebuild.rs                    后镜像校验 + 旧文件重建（纯函数，无 I/O）
src/evidence/mod.rs                     resolve() → Vec<ResolvedFile>（定 tier）
src/evidence/worktree.rs                Worktree：读盘 + .git 探测（无 git 依赖）
src/ast/mod.rs                          Language 枚举 + parse + ParsedFile
src/ast/scopes.rs                       enclosing_scope / definitions / kind_is_control_flow 等词表
src/ast/pair.rs                         新旧 definition 配对（供 Task 9/10/11 复用）
src/events/mod.rs                       EventSink + static DETECTORS + extract_events()
src/events/lex.rs                       token_stream / is_formatting_only
src/events/scope_diff.rs                definition 体内 statement-list 对齐骨架
src/events/file_level.rs                DETECTORS[0..2]
src/events/signature.rs                 DETECTORS[2..3]
src/events/statements.rs                DETECTORS[3..6]
src/events/leaves.rs                    DETECTORS[6..9]
src/events/fragment.rs                  hunk_only 路径
src/events/text_fallback.rs             text_fallback 路径
xtask/                                  独立 crate：calibrate/budget/gendev 三个本地开发命令
.github/workflows/ci.yml                三平台矩阵 + 四条门禁
tests/
  golden/*.diff + golden/*.events.json  金标准语料
  golden_worktree/**                    与 .diff 对应的工作树夹具（由 gendev 生成）
  fixtures/*.tsv                        Task 6 的 node-kind 探测表
  determinism.rs  patch_parse.rs  preimage.rs  evidence.rs  ast.rs  lex_fold.rs
  detectors_file.rs  detectors_signature.rs  detectors_statements.rs  detectors_leaves.rs
  fragment.rs  text_fallback.rs  budget.rs
scripts/check-no-network.sh  scripts/check-budget.sh
```

**体积纪律**：`golden_worktree/` 与 `fixtures/` 只进 `tests/`，**不得**用 `include_str!`/`include_dir!` 嵌进二进制，否则 12 MB 预算会被自己的测试夹具打爆。`check-budget` 会把体积打进 CI 摘要，便于回查是哪次提交顶上去的。

**依赖纪律**：`insta` / `proptest` / `tempfile` / `walkdir` 一律 dev-dependency；`xtask` 是 workspace 成员而非主 crate 依赖。主 crate 的 normal 依赖最终应为 `clap serde serde_json thiserror tree-sitter tree-sitter-javascript tree-sitter-typescript tree-sitter-python`（正好 8，若超需改 spec）。

---

### Task 1: 脚手架、CI 与四条门禁

覆盖 spec §12 的体积 / 冷启动 / 零网络三项 + §10 红线 + R3（Windows 构建）。放在最前是因为 R3 必须在第一天暴露，而不是留到分发阶段才发现构建不过。

**Files:**
- Create: `Cargo.toml`（workspace root + bin + lib）
- Create: `build.rs`
- Create: `src/main.rs`, `src/lib.rs`
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`
- Create: `.cargo/config.toml`
- Create: `scripts/check-no-network.sh`, `scripts/check-budget.sh`
- Create: `.github/workflows/ci.yml`
- Create: `rust-toolchain.toml`, `.gitignore`
- Test: `tests/budget.rs`

**Interfaces:**
- Consumes: 无
- Produces: `diffintent::PIPELINE_STAGES: &[&str]`；`diffintent_seir::EVENT_KIND_NAMES: [&str; 14]`；xtask 子命令 `gendev`（Task 3 填充实现）、`budget`、`calibrate`

- [ ] **Step 1: 确认 Windows 上具备 link.exe**

装 Rust **之前**先确认，否则会先花 2GB 下载再撞上模糊的 linker 错误。在 PowerShell 里：

```powershell
Get-Command link.exe -ErrorAction SilentlyContinue
Get-ChildItem "C:\Program Files*\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe" -ErrorAction SilentlyContinue
```

Expected: 至少一条路径。若无 → 安装 "Desktop development with C++" workload（含 MSVC v143 build tools + Windows SDK）。这是约 2–6 GB 的改动，属影响用户机器且不易逆转的操作，**先取得用户明确同意再动手**；可逆性更好的替代方案是 MinGW-w64（rustup `x86_64-pc-windows-gnu`），但那会偏离 spec §13 的 `win-x86_64` MSVC 发布形态。

- [ ] **Step 2: 装 Rust 并验证**

```powershell
winget install --id Rustlang.Rustup --accept-package-agreements --accept-source-agreements
```

开新终端（PATH 需要刷新）后：

```bash
rustup default stable && cargo --version && rustc --version
```

Expected: `cargo 1.9x.0` 与 `rustc 1.9x.0`。若 `cargo` 仍找不到，检查 `C:\Users\魏杰\.cargo\bin` 是否在 PATH。

- [ ] **Step 3: 建 workspace 与 crate 骨架**

`Cargo.toml`：

```toml
[workspace]
members = [".", "xtask"]

[package]
name = "diffintent"
version = "0.0.0"
edition = "2021"
rust-version = "1.77"
description = "Zero-LLM structural intent classification for unified diffs"
license = "MIT"
repository = "https://github.com/OWNER/diffintent"

[[bin]]
name = "diffintent"
path = "src/main.rs"

[lib]
name = "diffintent"
path = "src/lib.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[build-dependencies]

[dev-dependencies]
insta = "1"
proptest = "1"
tempfile = "3"
walkdir = "2"

[profile.release]
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

`rust-toolchain.toml`：

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`.gitignore`：

```
/target
Cargo.lock
```

（库形态也提交 lock 更好；这里按 bin crate 惯例提交 `Cargo.lock`，故把上面一行改成 `/target` 单独一行。**决定：提交 Cargo.lock**，因为发布物是二进制，可复现构建优先。）

- [ ] **Step 4: 写最小 lib 与 bin，让 workspace 能编译**

`src/lib.rs`：

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! diffintent：把 unified diff 压成意图差异。本计划只实现前三段流水线。

pub mod contract;
pub mod evidence;
pub mod patch;
pub mod seir;

/// 流水线七段的名字与顺序。W1 只实现前三段，其余报 Unsupported。
/// spec §4：段间只传 seir::Event 与 Block。
pub const PIPELINE_STAGES: &[&str] = &[
    "patch_parse",
    "evidence_resolve",
    "event_extract",
    "block_segment",
    "prior_score",
    "drift_align",
    "render",
];

/// W1 已实现的前缀长度。用于让 --json 诚实标注能力边界，而不是假装完整。
pub const IMPLEMENTED_STAGES: usize = 3;
```

`src/main.rs`：

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "diffintent", version, about = "intent-level diff analysis, zero LLM by construction")]
struct Cli {
    /// 从文件读 unified diff；缺省时读 stdin
    #[arg(long, value_name = "PATH")]
    file: Option<std::path::PathBuf>,
    /// 重建完整新旧文件的基线工作树；缺省取当前目录
    #[arg(long, value_name = "PATH")]
    repo: Option<std::path::PathBuf>,
    /// 原始请求摘要。有它才可能产出 drift: true（spec §9）
    #[arg(long, value_name = "TEXT")]
    request: Option<String>,
    /// 输出 diffintent/1 契约 JSON
    #[arg(long)]
    json: bool,
    /// 输出规范序原始结构事件（内部调试面，非契约，随时可变）
    #[arg(long)]
    dump_events: bool,
    /// 逐行内容
    #[arg(long)]
    details: bool,
    /// 存在 drift: true 块时退出码 1（spec §2.1）
    #[arg(long)]
    fail_on_drift: bool,
    /// 存在低置信或 unknown 块时退出码 2
    #[arg(long)]
    fail_on_unknown: bool,
}

fn main() {
    let cli = Cli::parse();
    // Task 14 填实。此处必须让 --version/--help 可用且尽快退出（冷启动预算的来源）。
    let _ = (cli.file, cli.repo, cli.request, cli.json, cli.dump_events);
    let _ = (cli.details, cli.fail_on_drift, cli.fail_on_unknown);
}
```

- [ ] **Step 5: 写 build.rs，把语法包 C 源从网络依赖里摘出来**

tree-sitter 语法 crate 的 build script 会在缺失时下载 `parser.c`/`scanner.c`。发布物不能带网络构建步骤，故在构建前把它们复制进 `vendor/grammar-src/` 并指向副本。

```rust
//! 复制所需 tree-sitter 语法 C 源到 OUT_DIR，并把环境变量指向副本。
//! 关键：只在构建前要求文件存在，绝不下载。
use std::{env, fs, path::Path};

const GRAMMAR_CRATES: &[(&str, &str)] = &[
    ("tree-sitter-javascript", "javascript"),
    ("tree-sitter-typescript", "typescript"),
    ("tree-sitter-python", "python"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = env::var("CARGO_MANIFEST_DIR")?;
    let out_dir = env::var("OUT_DIR")?;
    let lock = std::fs::read_to_string(format!("{manifest}/Cargo.lock"))?;

    for (krate, lang) in GRAMMAR_CRATES {
        println!("cargo:rerun-if-changed={krate}");
        let Some(ver) = parse_locked_version(&lock, krate) else {
            // Task 6 才加语法依赖；此前静默跳过，让 Task 1 能独立通过。
            continue;
        };
        let src_dir = Path::new(&env::var("CARGO_HOME").unwrap_or_else(|_| {
            format!("{}/.cargo", env::var("HOME").unwrap_or_default())
        }))
        .join("registry/src")
        .join(format!("{krate}-{ver}"));
        if !src_dir.exists() {
            return Err(format!("grammar src missing: {}", src_dir.display()).into());
        }
        let dst_dir = Path::new(&out_dir).join("grammar-src").join(lang);
        fs::create_dir_all(&dst_dir)?;
        for f in ["parser.c", "scanner.c", "scanner.h", "allocarray.c", "ucd_pattern.c", "array.h"] {
            let from = src_dir.join("src").join(f);
            if from.exists() {
                fs::copy(&from, dst_dir.join(f))?;
            }
        }
        // 让语法 crate 的 build script 使用本地副本而非下载
        println!("cargo:rustc-env=DIFFINTENT_GRAMMAR_SRC_{lang}= {}", dst_dir.display());
    }
    Ok(())
}

fn parse_locked_version(lock: &str, krate: &str) -> Option<String> {
    let mut in_block = false;
    for line in lock.lines() {
        if line.trim() == "[[package]]" { in_block = false; continue; }
        if in_block && let Some(v) = line.strip_prefix("version = \"").map(|s| s.trim_end_matches('"').to_string()) {
            return Some(v);
        }
        if line == format!("name = \"{krate}\"") { in_block = true; }
    }
    None
}
```

**注意**：`parse_locked_version` 的 `in_block` 逻辑很脆，Step 6 会先为它写单测再放行。若 `DIFFINTENT_GRAMMAR_SRC_*` 环境变量名与语法 crate build script 实际读取的名字不符（各 crate 版本不同），Task 6 的 Step 会实测并改这个名字——**不要在此处猜测环境变量名**。

- [ ] **Step 6: 为 lockfile 解析写测试**

`build.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::parse_locked_version;
    #[test]
    fn reads_version_for_named_package() {
        let lock = "\
[[package]]
name = \"serde\"
version = \"1.0.219\"

[[package]]
name = \"tree-sitter-python\"
version = \"0.25.0\"
";
        assert_eq!(parse_locked_version(lock, "tree-sitter-python").as_deref(), Some("0.25.0"));
        assert_eq!(parse_locked_version(lock, "serde").as_deref(), Some("1.0.219"));
    }
    #[test]
    fn returns_none_for_absent_package() {
        assert_eq!(parse_locked_version("name = \"x\"\n", "tree-sitter-go"), None);
    }
}
```

Run: `cargo test --build -p diffintent` 或 `cargo test`。Expected: 2 passed。

- [ ] **Step 7: Windows 编译器查找兜底**

`.cargo/config.toml`：

```toml
[target.x86_64-pc-windows-msvc]
# 允许 CI 把 CC 指到 cl.exe；本地从 VS Developer Shell 继承。
# 未设置时 cl.exe 不在 PATH 会让 tree-sitter 语法构建失败（spec R3）。
linker = "link.exe"
```

若 Step 1 选了 MinGW 路线则改为 `[target.x86_64-pc-windows-gnu] rustflags = ["-C", "target-cpu=native"]` 并在 spec §13 标注该偏差。

- [ ] **Step 8: 写禁网门禁**

`scripts/check-no-network.sh`：

```bash
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
```

- [ ] **Step 9: 写体积与冷启动门禁**

`scripts/check-budget.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail
EXE="${1:-target/release/diffintent}"
[ -f "$EXE" ] || { echo "二进制不存在: $EXE"; exit 1; }
if [ "$(uname -s 2>/dev/null || echo Windows)" = "Windows" ]; then EXE="$EXE.exe"; fi

SIZE=$(stat -c%s "$EXE" 2>/dev/null || wc -c < "$EXE")
echo "size: $((SIZE / 1024 / 1024)) MB ($SIZE bytes)  预算 < 12 MB"
[ "$SIZE" -lt $((12 * 1024 * 1024)) ] || { echo "FAIL: 超体积预算"; exit 1; }

# 冷启动：--version 走 clap 早退路径，不进流水线
BEST=999999
for _ in 1 2 3 4 5; do
  S=$(python -c "import time,subprocess;t=time.perf_counter();subprocess.run(['$EXE','--version'],stdout=subprocess.DEVNULL);print(int((time.perf_counter()-t)*1000))")
  [ "$S" -lt "$BEST" ] && BEST=$S
done
echo "cold start (best of 5): ${BEST} ms  预算 < 80 ms（CI 判定 160 ms）"
LIMIT=160
[ "$BEST" -lt "$LIMIT" ] || { echo "FAIL: 冷启动 ${BEST} ms 超 ${LIMIT} ms"; exit 1; }
echo "OK: 预算达标"
```

- [ ] **Step 10: 写 50 文件 / 2000 行的性能回归测试（W1 先空跑）**

`tests/budget.rs`：

```rust
use std::time::Instant;

/// 生成 50 文件、约 2000 行变更的 diff。spec §12：< 300 ms（CI 判定 600 ms）。
fn synthetic_diff(files: usize, lines_per_file: usize) -> String {
    let mut out = String::new();
    for f in 0..files {
        out.push_str(&format!(
            "diff --git a/src/m{f}.ts b/src/m{f}.ts\nindex 0000000..1111111 100644\n--- a/src/m{f}.ts\n+++ b/src/m{f}.ts\n@@ -1,{l} +1,{l} @@\n",
            l = lines_per_file
        ));
        for l in 0..lines_per_file {
            if l % 4 == 0 {
                out.push_str(&format!("-const v{f}_{l} = {l};\n"));
            } else {
                out.push_str(&format!("+const v{f}_{l} = {};\n", l + 1));
            }
        }
    }
    out
}

#[test]
#[ignore = "需 --release；由 scripts/check-budget.sh 与 CI 调用"]
fn fifty_file_two_thousand_line_under_600ms() {
    let text = synthetic_diff(50, 20); // 50 文件 × 20 行 × 双向 ≈ 2000 行变更
    let start = Instant::now();
    let parsed = diffintent::patch::parse(&text).expect("parse");
    let events = diffintent::events::extract_events(&parsed, &diffintent::evidence::Worktree::none())
        .expect("extract");
    let ms = start.elapsed().as_millis();
    println!("50f/2k diff: {ms} ms, {} events", events.len());
    assert!(ms < 600, "超预算: {ms} ms");
}
```

（`diffintent::events` 与 `Worktree::none()` 在 Task 5 / 8 才存在。此测试标 `#[ignore]`，**Task 1 不要求它编译通过**——Step 11 的 CI 用 `cargo test`（不含 ignore）。Task 14 Step 把它接上并解除 ignore。）

- [ ] **Step 11: 写三平台 CI**

`.github/workflows/ci.yml`：

```yaml
name: ci
on:
  push: { branches: [main] }
  pull_request:
permissions:
  contents: read
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    defaults:
      run: { shell: bash }
    env:
      CARGO_TERM_COLOR: always
    steps:
      - uses: actions/checkout@v4
      - uses: rust-cache@v1   # 若该 action 不存在则改用 Swatinem/rust-cache@v2
      - run: rustup component add rustfmt clippy
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets --locked -- -D warnings -D clippy::all
      - run: cargo test --locked
      - name: 禁网门禁（spec §10）
        run: bash scripts/check-no-network.sh
      - run: cargo build --release --locked
      - name: 体积与冷启动门禁（spec §12）
        run: bash scripts/check-budget.sh target/release/diffintent
      - name: 性能基准
        run: cargo test --release --locked -- --ignored --cases fifty_file
      - name: 体积写入摘要（便于回查是哪次提交顶上去的）
        if: always()
        run: |
          SIZE=$(stat -c%s target/release/diffintent 2>/dev/null || wc -c < target/release/diffintent.exe)
          echo "diffintent release size: $((SIZE/1024/1024)) MB" >> "$GITHUB_STEP_SUMMARY"
```

`lto = "fat"` 会显著拖慢 CI；此处先用 `thin`，Task 14 再评估是否值得换 fat 换体积。

- [ ] **Step 12: xtask 骨架（永不进发布物）**

`xtask/Cargo.toml`：

```toml
[package]
name = "xtask"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
clap = { version = "4", features = ["derive"] }
```

`xtask/src/main.rs`：

```rust
//! 本地开发命令。绝不进发布物、不进 README 的安装路径。
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Xtask { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    /// 从 tests/golden/*.diff 生成 tests/golden_worktree/（开发者命令；CI 只消费已提交夹具）
    Gendev,
    /// 由评测集生成 src/calibration.rs（W4）
    Calibrate,
    /// 跑体积与冷启动预算
    Budget,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Xtask::parse().cmd {
        Cmd::Gendev => println!("implemented in Task 3"),
        Cmd::Calibrate => Err("W4 实现（spec §8.2）".into()),
        Cmd::Budget => Err("Task 14 接线".into()),
    }
}
```

（`Cmd::Budget` 在 Task 1 返回 Err 会让 `cargo test -p xtask` 失败吗？不会——只编译不执行。但 **`cargo clippy` 会警告 unreachable match**，故此处保持返回 Err 而非 todo!()。）

- [ ] **Step 13: 验证并跑门禁**

```bash
cargo build && cargo test && bash scripts/check-no-network.sh && bash scripts/check-budget.sh target/debug/diffintent
```

Expected: 全绿。`check-budget` 对 debug 二进制会报超体积——**这正常**，Step 14 之前先 `cargo build --release` 再跑。debug 体积不是门禁项。

- [ ] **Step 14: 提交**

```bash
git checkout -b feat/w1-scaffold
git add Cargo.toml Cargo.lock build.rs .cargo/config.toml rust-toolchain.toml .gitignore \
        src/main.rs src/lib.rs xtask scripts .github
git commit -m "feat: Rust 脚手架 + 三平台 CI + 禁网/体积/冷启动门禁

build.rs 把语法包 C 源从网络构建路径摘出（发布物不得有下载步骤）。
check-no-network.sh 落实 spec §10 的构造级禁网断言。
Windows MSVC 从第一天进 CI 矩阵，对应 spec R3。"
```

创建 PR 并等三平台全绿后再继续。**若 `windows-latest` 挂在 link.exe，即为 R3 命中**：在 PR 里记录失败的具体 crate 与 build script 输出，不要靠加 `cc` wrapper 绕过——那会把问题推到分发阶段。

---

### Task 2: seir 核心类型与规范序

覆盖 spec §5、§4.1。这是整个产品的"唯一契约"（也是将来与 sift 共享的那份 IR），必须早定且带完整性质测试。

**Files:**
- Create: `src/seir/mod.rs`, `src/seir/tag.rs`, `src/seir/table.rs`
- Modify: `src/lib.rs`（`pub mod seir;` 已在 Task 1 声明，此处补 `EVENT_KIND_NAMES` re-export）
- Test: `tests/seir.rs`, `src/seir/mod.rs` 内 `#[cfg(test)]`

**Interfaces:**
- Consumes: Task 1 的 crate 与 lint 配置
- Produces:
  - `seir::EventKind`（14 变体，`Copy + Ord`，声明序即规范序）
  - `seir::Direction { Added, Removed, Modified }`
  - `seir::EvidenceTier { FullAst, HunkOnly, TextFallback }` + `as_str()`
  - `seir::ScopeRef { name: String, node_kind: String, range: (u32, u32) }`
  - `seir::FileId(pub u32)`
  - `seir::Event { file, kind, direction, scope, node_kind, range, tier }`
  - `seir::Event::tag(&self) -> String`（`"guard:added"`）
  - `seir::canonical_sort(&mut Vec<Event>)`（排序 + 去重）
  - `seir::table::FileTable::{from_paths, id_of, path_of, len}`
  - `seir::EVENT_KIND_NAMES: [&str; 14]`

- [ ] **Step 1: 写 kind 枚举与其字符串形式的测试**

`src/seir/mod.rs`：

```rust
//! 结构事件中间表示（SEIR）。spec §5：七段流水线唯一的段间契约。

use serde::Serialize;

/// 14 个结构事件类型。**变体顺序即规范序的一部分**，改动顺序属破坏性变更。
/// spec §5.1 是这张表的来源；加 kind 必须先改 spec。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Signature,
    Guard,
    ControlFlow,
    ErrorHandling,
    Expression,
    Formatting,
    Comment,
    Binding,
    Call,
    Dependency,
    Module,
    File,
    TestCase,
    ConfigKey,
}

/// spec §5.1 表格顺序。用于 --dump-events 与文档一致性断言。
pub const EVENT_KIND_NAMES: [&str; 14] = [
    "signature", "guard", "control_flow", "error_handling", "expression", "formatting",
    "comment", "binding", "call", "dependency", "module", "file", "test_case", "config_key",
];

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Guard => "guard",
            Self::ControlFlow => "control_flow",
            Self::ErrorHandling => "error_handling",
            Self::Expression => "expression",
            Self::Formatting => "formatting",
            Self::Comment => "comment",
            Self::Binding => "binding",
            Self::Call => "call",
            Self::Dependency => "dependency",
            Self::Module => "module",
            Self::File => "file",
            Self::TestCase => "test_case",
            Self::ConfigKey => "config_key",
        }
    }
}

#[cfg(test)]
mod kind_tests {
    use super::*;

    /// 防漂移：枚举 Display 与文档表必须逐项一致。顺序不同即失败。
    #[test]
    fn kind_names_match_declaration_order() {
        let all = [
            EventKind::Signature, EventKind::Guard, EventKind::ControlFlow, EventKind::ErrorHandling,
            EventKind::Expression, EventKind::Formatting, EventKind::Comment, EventKind::Binding,
            EventKind::Call, EventKind::Dependency, EventKind::Module, EventKind::File,
            EventKind::TestCase, EventKind::ConfigKey,
        ];
        let actual: Vec<&str> = all.iter().map(|k| k.as_str()).collect();
        assert_eq!(actual, EVENT_KIND_NAMES.to_vec());
        assert_eq!(all.len(), 14, "spec §5.1 固定 14 个 kind");
    }

    /// Ord 必须等于声明序，否则规范序不确定。
    #[test]
    fn kind_ord_is_declaration_order() {
        assert!(EventKind::Signature < EventKind::Guard);
        assert!(EventKind::ConfigKey > EventKind::TestCase);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --locked seir` → Expected: 编译失败（`Direction` 等尚未定义），或若 Step 1 自成一体则 PASS。**本任务 Step 1 已自洽，故预期 PASS**；若 FAIL 说明 Ord derive 与数组顺序不符，先修再往下。

- [ ] **Step 3: 补方向、层级、Event 与 tag**

同文件继续追加：

```rust
/// 事件方向。spec §5：方向不可丢失——`error_handling:removed` 是最该报的信号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Added,
    Removed,
    Modified,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
        }
    }
}

/// spec §6 三级证据降级链。声明序 = 证据强度降序，供 tier 打折直接比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    FullAst,
    HunkOnly,
    TextFallback,
}

impl EvidenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FullAst => "full_ast",
            Self::HunkOnly => "hunk_only",
            Self::TextFallback => "text_fallback",
        }
    }
}

/// 所属定义。spec §5 的 ScopeRef。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ScopeRef {
    pub name: String,
    pub node_kind: String,
    /// 新文件的 1-based 闭开区间
    pub range: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct FileId(pub u32);

/// 单个结构事件。spec §5 的 Event。字段顺序即 serde 输出顺序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub file: FileId,
    pub kind: EventKind,
    pub direction: Direction,
    pub scope: Option<ScopeRef>,
    pub node_kind: String,
    pub range: (u32, u32),
    pub tier: EvidenceTier,
}

impl Event {
    /// 契约里的 `structural_events` 字符串形式（spec §3.1）。
    pub fn tag(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.direction.as_str())
    }

    /// 规范序排序键。含全部字段以保证**同键事件也确定**（spec §4.1）。
    fn sort_key(&self) -> (u32, u32, u32, EventKind, Direction, &str, &str, u8) {
        (
            self.file.0,
            self.range.0,
            self.range.1,
            self.kind,
            self.direction,
            self.node_kind.as_str(),
            self.scope.as_ref().map(|s| s.name.as_str()).unwrap_or(""),
            match &self.scope { Some(_) => 1, None => 0 },
        )
    }
}

/// spec §4.1 的可测不变量：进入第 5 段前，所有集合必须按此排序并去重。
pub fn canonical_sort(events: &mut Vec<Event>) {
    events.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    events.dedup_by(|a, b| a.sort_key() == b.sort_key());
}
```

- [ ] **Step 4: 写规范序的性质测试（打乱输入必须同输出）**

`tests/seir.rs`：

```rust
use diffintent::seir::*;

fn ev(file: u32, kind: EventKind, dir: Direction, start: u32, node: &str) -> Event {
    Event {
        file: FileId(file),
        kind,
        dir_is: dir,
        range: (start, start + 4),
        node_kind: node.into(),
        scope: None,
        tier: EvidenceTier::FullAst,
    }
}
```

**修正上面笔误后再继续**（`dir_is` 应为 `direction`）：

```rust
fn ev(file: u32, kind: EventKind, direction: Direction, start: u32, node: &str) -> Event {
    Event {
        file: FileId(file),
        kind,
        direction,
        range: (start, start + 4),
        node_kind: node.into(),
        scope: None,
        tier: EvidenceTier::FullAst,
    }
}

fn sample() -> Vec<Event> {
    vec![
        ev(0, EventKind::Guard, Direction::Added, 12, "if_statement"),
        ev(0, EventKind::ControlFlow, Direction::Added, 12, "if_statement"),
        ev(1, EventKind::Dependency, Direction::Added, 3, "import_statement"),
        ev(0, EventKind::ErrorHandling, Direction::Removed, 40, "try_statement"),
        ev(1, EventKind::Module, Direction::Added, 1, "class_declaration"),
    ]
}

#[test]
fn canonical_sort_is_permutation_independent() {
    let mut a = sample();
    let mut b = sample();
    b.reverse();
    canonical_sort(&mut a);
    canonical_sort(&mut b);
    assert_eq!(a, b);
}

#[test]
fn canonical_order_is_file_then_range_then_kind() {
    let mut v = sample();
    canonical_sort(&mut v);
    let keys: Vec<(u32, u32, &str)> = v
        .iter()
        .map(|e| (e.file.0, e.range.0, e.tag().as_str()))
        .collect();
    assert_eq!(
        keys,
        vec![
            (0, 12, "control_flow:added"),
            (0, 12, "guard:added"),
            (0, 40, "error_handling:removed"),
            (1, 1, "module:added"),
            (1, 3, "dependency:added"),
        ]
    );
}

/// spec §4.1：排序 + 去重必须幂等，且重复事件收敛为一条。
#[test]
fn canonical_sort_is_idempotent_and_dedupes() {
    let mut v = sample();
    let mut dup = v.clone();
    dup.extend(sample());
    canonical_sort(&mut dup);
    canonical_sort(&mut v);
    assert_eq!(v, dup);
}

#[test]
fn tag_carries_direction() {
    let e = ev(0, EventKind::ErrorHandling, Direction::Removed, 5, "try_statement");
    assert_eq!(e.tag(), "error_handling:removed");
}
```

Run: `cargo test --locked` → Expected: 上述测试全 PASS。

- [ ] **Step 5: 用 proptest 锁住"任意排列同结果"**

`tests/seir.rs` 追加：

```rust
use proptest::prelude::*;

fn arb_event(idx in 0u32..3) -> impl Strategy<Value = Event> {
    (0u32..3, 0u32..60, "[a-z_]{3,12}").prop_map(move |(kind_ix, start, node)| {
        let kinds = [EventKind::Guard, EventKind::ControlFlow, EventKind::Dependency];
        Event {
            file: FileId(idx),
            kind: kinds[kind_ix as usize % 3],
            direction: Direction::Added,
            range: (start, start + 1),
            node_kind: node,
            scope: None,
            tier: EvidenceTier::FullAst,
        }
    })
}

proptest! {
    #[test]
    fn any_permutation_yields_identical_bytes(
        events in prop::collection::vec((0u32..3).prop_flat_map(arb_event), 0..40),
        seed in any::<u64>(),
    ) {
        let mut a = events.clone();
        let mut b = events.clone();
        // 用确定性伪随机打乱，避免 proptest 里用 rand crate
        let mut s = seed;
        for i in (1..b.len()).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (s >> 33) as usize % (i + 1);
            b.swap(i, j);
        }
        canonical_sort(&mut a);
        canonical_sort(&mut b);
        let sa = serde_json::to_string(&a).unwrap();
        let sb = serde_json::to_string(&b).unwrap();
        prop_assert_eq!(sa, sb);
    }
}
```

Run: `cargo test --locked proptest` → Expected: PASS（256 用例）。

- [ ] **Step 6: FileTable 按路径字典序赋值**

`src/seir/table.rs`：

```rust
//! 路径 → FileId。**按路径字典序赋值，不按遇到顺序**（spec §4.1 确定性）。
//! 若按遇到顺序赋值，输入文件顺序一打乱 FileId 就变，输出跟着变。

use super::FileId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTable {
    paths: Vec<String>,
}

impl FileTable {
    /// 从任意顺序的路径集合构造；重复路径收敛为一条。
    pub fn from_paths<I: IntoIterator<Item = String>>(paths: I) -> Self {
        let mut v: Vec<String> = paths.into_iter().collect();
        v.sort();
        v.dedup();
        Self { paths: v }
    }

    pub fn id_of(&self, path: &str) -> Option<FileId> {
        self.paths.binary_search(&path.to_string()).ok().map(|i| FileId(i as u32))
    }

    pub fn path_of(&self, id: FileId) -> Option<&str> {
        self.paths.get(id.0 as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// 规范序遍历顺序（Task 8 的 extract_events 用它，禁止用 HashMap 迭代）。
    pub fn iter(&self) -> impl Iterator<Item = (FileId, &str)> {
        self.paths.iter().enumerate().map(|(i, p)| (FileId(i as u32), p.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_is_input_order_independent() {
        let a = FileTable::from_paths(["src/b.ts", "src/a.ts", "src/c.ts"].map(String::from));
        let b = FileTable::from_paths(["src/c.ts", "src/a.ts", "src/b.ts"].map(String::from));
        assert_eq!(a, b);
        assert_eq!(a.id_of("src/a.ts"), Some(FileId(0)));
        assert_eq!(a.id_of("src/c.ts"), Some(FileId(2)));
    }

    #[test]
    fn duplicates_collapse() {
        let t = FileTable::from_paths(["x.ts", "x.ts"].map(String::from));
        assert_eq!(t.len(), 1);
    }

    /// 删除文件用 `---` 侧路径；重命名涉及两个路径。两者都必须进表。
    #[test]
    fn missing_path_is_none_not_panic() {
        let t = FileTable::from_paths(["a.ts"].map(String::from));
        assert_eq!(t.id_of("zzz.ts"), None);
    }
}
```

- [ ] **Step 7: 接线并验证**

`src/seir/mod.rs` 顶部补：

```rust
pub mod table;
pub mod tag;
```

`src/lib.rs` 补 re-export，让 `--dump-events` 与文档能拿到 kind 表：

```rust
pub use seir::EVENT_KIND_NAMES;
```

Run: `cargo test --locked && cargo clippy --all-targets -- -D warnings` → Expected: 全绿。

- [ ] **Step 8: 提交**

```bash
git add src/seir tests/seir src/lib.rs
git commit -m "feat: SEIR 结构事件类型与规范序

FileTable 按路径字典序赋值而非遇到顺序，否则打乱输入文件顺序会让
FileId 漂移、输出不再逐字节相同（spec §4.1）。
proptest 锁住任意排列同结果。"
```

---

### Task 3: unified diff 解析

覆盖 spec §4 第 1 段。`patch_parse` 不丢信息：行号、前后缀、rename 元数据、`mode` 变化全保留。rename 元数据是 Task 7 "折叠"判据的唯一来源（spec §5.2 明确不做 AST 级 moved 检测）。

**Files:**
- Create: `src/patch/mod.rs`, `src/patch/model.rs`, `src/patch/parse.rs`
- Create: `tests/golden/rename.diff`, `tests/golden/new_and_deleted.diff`, `tests/golden/multi_hunk.diff`, `tests/golden/no_newline.diff`, `tests/golden/crlf.diff`
- Create: `xtask/src/gendev.rs`
- Test: `tests/patch_parse.rs`

**Interfaces:**
- Consumes: Task 2 的 `seir`
- Produces:
  - `patch::FileDiff { old_path: Option<String>, new_path: Option<String>, change: FileChange, hunks: Vec<Hunk>, binary: bool }`
  - `patch::FileChange { Added, Deleted, Renamed { from_similarity: u8 }, Modified, ModeOnly }`
  - `patch::Hunk { old_start: u32, old_count: usize, new_start: u32, new_count: usize, lines: Vec<DiffLine>, header_text: Option<String> }`
  - `patch::DiffLine { Context(String), Added(String), Removed(String), NoNewline }`
  - `patch::parse(text: &str) -> Result<Vec<FileDiff>, PatchError>`
  - `FileDiff::newest_path() -> &str` / `oldest_path() -> &str`
  - `xtask gendev`

- [ ] **Step 1: 先造真实夹具（不要用手工拼的 diff）**

手工拼的 diff 会漏掉 `index` 行、`similarity index`、`\ No newline` 这些真实形态。用真 git 生成：

```bash
mkdir -p tests/golden
TMP=$(mktemp -d) && cd "$TMP" && git init -q && git config user.email t@t && git config user.name t
mkdir -p src && printf 'export function a() {\n  return 1;\n}\n' > src/a.ts
printf 'export const c = 2;\n' > src/c.ts
printf 'line1\nline2\nline3' > src/n.txt
printf 'export const d = 4;\n' > src/d.ts
printf 'x=1\ny=2\n' > src/p.py
git add -A && git commit -qm base
# 改名 + 微改（产生 similarity index + rename from/to）
git mv src/c.ts src/renamed.ts && printf 'export const c = 3;\n' > src/renamed.ts
# 新增
printf 'export class New { m() { return 1 } }\n' > src/new.ts
# 删除
git rm -q src/d.ts
# 多 hunk 修改
printf 'export function a() {\n  if (!x) return 0;\n  return 1;\n}\n\nexport function b() {\n  return 2;\n}\n' > src/a.ts
# 无换行结尾
printf 'line1\nline2\nchanged' > src/n.txt
# CRLF
printf 'using System;\r\nnamespace N {\r\n  class C {}\r\n}\r\n' > src/cs.cs
git add -A && git commit -qm change
git diff HEAD~1 > ../../../tests/golden/mixed.diff
cd - && rm -rf "$TMP"
```

然后按主题切成四个夹具（每个都必须是 `git diff` 的真实输出，不要手写）：

```bash
cd tests/golden
# 用 mixed.diff 校验整体；再按文件切出专项夹具
awk '/^diff --git a\/src\/renamed.ts/,/^diff --git a\/src\/(new|d|a|n)/' mixed.diff | head -n -1 > rename.diff
```

若 `awk` 切分在你手上不干净，**不要手改内容**：直接在临时 git 仓里针对单个文件做操作并 `git diff HEAD~1 -- <path>` 输出到对应夹具文件。这是本任务唯一允许的夹具生成方式。

Expected: `tests/golden/` 下有 `mixed.diff`、`rename.diff`、`new_and_deleted.diff`、`multi_hunk.diff`、`no_newline.diff`、`crlf.diff`。`rename.diff` 必须含 `similarity index` 与 `rename from` / `rename to` 行——若没有，说明改动比例过高被 git 当成 delete+add，把改动缩到一行再重生成。

- [ ] **Step 2: 写失败测试**

`tests/patch_parse.rs`：

```rust
use diffintent::patch::{DiffLine, FileChange, *};

fn load(name: &str) -> Vec<FileDiff> {
    let text = std::fs::read_to_string(format!("tests/golden/{name}")).expect("fixture");
    parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn parses_every_file_in_mixed_diff() {
    let files = load("mixed.diff");
    assert_eq!(files.len(), 6, "a.ts renamed.ts d.ts new.ts n.txt cs.cs");
    // 路径必须来自 --- +++ 行，不是 diff --git 行（前者带 a/ b/ 前缀且可被引号包裹）
    let mut paths: Vec<&str> = files.iter().map(|f| f.newest_path()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec!["src/a.ts", "src/cs.cs", "src/n.txt", "src/new.ts", "src/p.py", "src/renamed.ts"]
    );
}

#[test]
fn rename_preserves_metadata_from_git_lines() {
    let files = load("rename.diff");
    let f = &files[0];
    assert!(matches!(f.change, FileChange::Renamed { .. }), "{:?}", f.change);
    assert_eq!(f.oldest_path(), Some("src/c.ts"));
    assert_eq!(f.newest_path(), "src/renamed.ts");
}

#[test]
fn added_and_deleted_are_distinguished() {
    let files = load("new_and_deleted.diff");
    let added = files.iter().find(|f| f.newest_path() == "src/new.ts").unwrap();
    assert_eq!(added.change, FileChange::Added);
    assert_eq!(added.oldest_path(), None);
    let deleted = files.iter().find(|f| f.oldest_path() == Some("src/d.ts")).unwrap();
    assert_eq!(deleted.change, FileChange::Deleted);
    assert_eq!(deleted.newest_path(), "");
}

/// 行号必须可直接用于磁盘校验，不能有 off-by-one。
#[test]
fn hunk_line_numbers_are_exact() {
    let files = load("multi_hunk.diff");
    let a = files.iter().find(|f| f.newest_path() == "src/a.ts").unwrap();
    assert_eq!(a.hunks.len(), 2, "两个分离的 hunk");
    assert_eq!(a.hunks[0].new_start, 1);
    assert!(a.hunks[1].new_start > a.hunks[0].new_start);
    let old_total: usize = a.hunks.iter().map(|h| h.old_count).sum::<usize>()
        + (a.hunks[1].new_start as usize - (a.hunks[0].new_start as usize + a.hunks[0].new_count)) ;
    assert!(old_total > 0);
}

/// hunk 内行数必须自洽，否则 Task 4 的行位置校验会全线错位。
#[test]
fn hunk_line_counts_match_body() {
    for name in ["mixed.diff", "multi_hunk.diff", "new_and_deleted.diff"] {
        for f in load(name) {
            for h in &f.hunks {
                let ctx = h.lines.iter().filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Removed(_))).count();
                let new = h.lines.iter().filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Added(_))).count();
                assert_eq!(ctx, h.old_count, "{name} {} old_count", f.newest_path());
                assert_eq!(new, h.new_count, "{name} {} new_count", f.newest_path());
            }
        }
    }
}

/// `\ No newline at end of file` 不能变成内容行，否则内容比对必然失配。
#[test]
fn no_newline_marker_is_not_a_content_line() {
    let files = load("no_newline.diff");
    let n = files.iter().find(|f| f.newest_path() == "src/n.txt").unwrap();
    assert!(n.hunks.iter().any(|h| h.lines.iter().any(|l| matches!(l, DiffLine::NoNewline))));
    for h in &n.hunks {
        let content = h.lines.iter().filter(|l| !matches!(l, DiffLine::NoNewline)).count();
        let ctx = h.lines.iter().filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Removed(_))).count();
        assert_eq!(ctx, h.old_count, "排除 NoNewline 后必须自洽");
        let _ = content;
    }
}

/// 空文件 diff（仅 mode 变化 / 空 diff）不得报错，也不得产出幻影 hunk。
#[test]
fn mode_only_and_empty_input_are_ok() {
    let files = parse("diff --git a/x.ts b/x.ts\nold mode 100644\nnew mode 100755\n").unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].change, FileChange::ModeOnly);
    assert!(files[0].hunks.is_empty());
    assert_eq!(parse("").unwrap(), vec![]);
}

/// 非法输入必须报退出码 3 的错误类，而不是静默产出空结果。
#[test]
fn rejects_non_unified_input() {
    let err = parse("On branch main\nnothing to commit\n").unwrap_err();
    assert!(matches!(err, PatchError::NotADiff), "{err:?}");
}

/// 路径含空格与中文时 git 会加引号，必须正确解出。
#[test]
fn handles_quoted_paths_with_spaces_and_cjk() {
    let text = "diff --git a/src/my file.ts b/src/my file.ts\n\
                index 0000000..1111111 100644\n\
                --- a/src/my file.ts\n\
                +++ b/src/my file.ts\n\
                @@ -1 +1 @@\n-a\n+b\n";
    let files = parse(text).unwrap();
    assert_eq!(files[0].newest_path(), "src/my file.ts");
}
```

Run: `cargo test --locked --test patch_parse` → Expected: 编译失败（`patch` 模块无这些项）。

- [ ] **Step 3: 数据模型**

`src/patch/model.rs`：

```rust
//! unified diff 的无损表示。spec §4 第 1 段：行号、前后缀、rename 元数据全保留。

/// 单行变更。`NoNewline` 是 `\ No newline at end of file`，**不是内容行**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
    NoNewline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    Added,
    Deleted,
    Renamed { from_similarity: u8 },
    Modified,
    ModeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// @@ 头的 1-based 起点与行数。0 起点表示空文件（新增/删除）。
    pub old_start: u32,
    pub old_count: usize,
    pub new_start: u32,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
    /// @@ ... @@ 之后的函数上下文提示。**不可靠**（git 对 TS 的 hunk header 正则很弱），
    /// 仅可用于 reason 文本辅助，绝不得参与 scope 判定或 drift 判定。spec §6 的 scope 来自 AST。
    pub header_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// `---` 侧路径，已去 `a/` 前缀。`/dev/null` → None。
    pub old_path: Option<String>,
    /// `+++` 侧路径，已去 `b/` 前缀。`/dev/null` → None。
    pub new_path: Option<String>,
    pub change: FileChange,
    pub hunks: Vec<Hunk>,
    pub binary: bool,
    /// diff 是否带 `diff --git` 扩展头。纯 `patch -p1` 产物没有它，
    /// 此时 change 只能推断为 Modified，rename 无法识别（Task 7 因此不会折叠）。
    pub has_git_extension_header: bool,
}

impl FileDiff {
    /// 用于读盘与 FileTable 赋值。删除文件返回 ""（其磁盘对应物不存在）。
    pub fn newest_path(&self) -> &str {
        self.new_path.as_deref().unwrap_or("")
    }

    pub fn oldest_path(&self) -> Option<&str> {
        self.old_path.as_deref()
    }

    /// 磁盘上应存在哪一个文件、对应哪一侧。
    pub fn disk_side(&self) -> DiskSide {
        match self.change {
            FileChange::Added => DiskSide::NewExists,
            FileChange::Deleted => DiskSide::OldExists,
            FileChange::Renamed { .. } | FileChange::Modified | FileChange::ModeOnly => DiskSide::NewExists,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskSide {
    /// 工作树里是 b/ 侧（`git diff` / `git diff HEAD` 的常规情形）
    NewExists,
    /// 工作树里只有 a/ 侧（删除文件）
    OldExists,
}
```

- [ ] **Step 4: 解析器**

`src/patch/parse.rs`：

```rust
//! unified diff → Vec<FileDiff>。

use super::{DiffLine, FileChange, FileDiff, Hunk, PatchError};

pub fn parse(text: &str) -> Result<Vec<FileDiff>, PatchError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(vec![]);
    }
    let mut out: Vec<FileDiff> = Vec::new();
    let mut i = 0;
    let mut saw_git_header = false;
    let mut saw_content = false;

    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix("diff --git ") {
            saw_git_header = true;
            saw_content = true;
            let (f, next) = parse_one(&lines, i, rest)?;
            out.push(f);
            i = next;
            continue;
        }
        // 无 git 扩展头的 unified diff（`patch -p1` / `git diff --no-index` 变体）
        if line.starts_with("--- ") {
            saw_content = true;
            let (f, next) = parse_one_no_extension(&lines, i)?;
            out.push(f);
            i = next;
            continue;
        }
        if line.is_empty() || line.starts_with("index ") || line.starts_with("new mode")
            || line.starts_with("old mode") || line.starts_with("similarity index")
            || line.starts_with("rename from") || line.starts_with("rename to")
            || line.starts_with("copy from") || line.starts_with("copy to")
            || line.starts_with("Binary files") || line.starts_with("--- ")
            || line.starts_with("+++ ") || line.starts_with("@@") {
            return Err(PatchError::StrayLine(i));
        }
        i += 1;
    }

    if !saw_content {
        return Err(PatchError::NotADiff);
    }
    Ok(out)
}

/// git 对含空格/非 ASCII 的路径加双引号并做 C 风格转义（默认 `core.quotepath=true` 时
/// 非 ASCII 走八进制，例如中文 `你` 输出为 `\344\275\240`）。必须还原成 UTF-8 字节，
/// 否则 `+++ b/src/你.ts` 这类路径会被解成乱码，读盘必然 FileMissing。
fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if !(t.len() >= 2 && t.starts_with('"') && t.ends_with('"')) {
        return t.to_string();
    }
    let inner = &t[1..t.len() - 1];
    let mut bytes: Vec<u8> = Vec::with_capacity(inner.len());
    let mut it = inner.bytes();
    while let Some(b) = it.next() {
        if b != b'\\' {
            bytes.push(b);
            continue;
        }
        match it.next() {
            Some(d @ b'0'..=b'7') => {
                let mut v = (d - b'0') as u32;
                for _ in 0..2 {
                    if let Some(n @ b'0'..=b'7') = it.peek().copied() {
                        v = v * 8 + (n - b'0') as u32;
                        it.next();
                    } else {
                        break;
                    }
                }
                bytes.push(v as u8);
            }
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(b'"') => bytes.push(b'"'),
            Some(b'\\') => bytes.push(b'\\'),
            Some(other) => bytes.push(other),
            None => break,
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 去掉 a/ b/ 前缀；`/dev/null` → None
fn path_from_side(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let p = unquote(rest.split("	").next().unwrap_or(rest).trim());
    if p == "/dev/null" { return None; }
    let p = p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(&p).to_string();
    Some(p)
}

fn parse_hunk_header(line: &str) -> Option<(u32, usize, u32, usize, Option<String>)> {
    // @@ -l,s +l,s @@ optional
    // s 省略时默认 1；起点 0 表示空文件
    let rest = line.strip_prefix("@@ ")?;
    let (old_part, tail) = rest.split_once(" +")?;
    let (new_part, ctx) = tail.split_once(" @@").unwrap_or((tail, None));
    let old_start = old_part.strip_prefix('-')?.split_once(',')
        .map(|(a, _)| a)
        .unwrap_or(old_part.strip_prefix('-')?);
    let old_count = old_part.split_once(',').map(|(_, b)| b).unwrap_or("1");
    let parse = |s: &str| s.trim().parse::<i64>().ok();
    let os = parse(old_start)?;
    let oc = parse(old_count)?;
    let (ns, nc) = match new_part.split_once(',') {
        Some((a, b)) => (parse(a)?, parse(b)?),
        None => (parse(new_part)?, 1),
    };
    Some((
        os.max(0) as u32,
        oc.max(0) as usize,
        ns.max(0) as u32,
        nc.max(0) as usize,
        ctx.map(str::to_string),
    ))
}
```

`path_from_side` 处理 `--- a/src/x.ts	TIMESTAMP` 这种带 tab 分隔时间戳的形态（`patch -p1` 常见），以及 `+++ b/x.ts` 后跟多个空格再跟时间的形态。**必须按 tab 优先、否则整体去尾空白**，否则路径会带上时间戳尾巴导致读盘必然 FileMissing。`tests/patch_parse.rs` 需补一条带 tab 时间戳的断言。

补测试到 `tests/patch_parse.rs`：

```rust
#[test]
fn unquotes_octal_cjk_path() {
    // git 默认 core.quotepath=true 时中文路径输出八进制
    let text = "diff --git a/src/\"\\344\\275\\240.ts\" b/src/\"\\344\\275\\240.ts\"\n\
                index 0000000..1111111 100644\n\
                --- a/src/\"\\344\\275\\240.ts\"\n\
                +++ b/src/\"\\344\\275\\240.ts\"\n\
                @@ -1 +1 @@\n-a\n+b\n";
    let files = parse(text).unwrap();
    assert_eq!(files[0].newest_path(), "src/你.ts");
}
```

（`你` 的 UTF-8 是 `\344\275\240`。若 Step 1 的真实 git 输出用的是别的转义形式，以真实输出为准改写期望值，**不要改实现去迎合**。可先 `git -c core.quotepath=false diff` 生成不转义的夹具，并把两种形态各留一个测试。）

`src/patch/mod.rs`：

```rust
mod model;
mod parse;
mod rebuild;

pub use model::*;
pub use parse::parse;
pub use rebuild::{reconstruct_old, verify_disk, DiskVerdict};

/// spec §2.1 退出码 3 对应的错误类族。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PatchError {
    #[error("input is not a unified diff")]
    NotADiff,
    #[error("line {0}: diff content outside any hunk")]
    StrayLine(usize),
    #[error("line {0}: malformed hunk header")]
    BadHunkHeader(usize),
    #[error("line {0}: hunk declares {decl} lines but body has {actual}")]
    HunkLengthMismatch { line: usize, decl: usize, actual: usize },
    #[error("file `{0}`: missing `---`/`+++` header")]
    MissingSideHeader(String),
}
```

（`parse_one` / `parse_one_no_extension` 未在上面给出——它们是 Step 4 剩下的部分：读 `diff --git` 后的扩展行（`similarity index NN%` / `rename from` / `rename to` / `Binary files` / `old mode` / `new mode`），再读 `---`/`+++`，再循环读 hunk 直到遇到下一个 `diff --git` 或 `index ` 或输入结束。每个 hunk 读满 `old_count`/`new_count` 对应的行数，行数不符即返回 `HunkLengthMismatch`。**这两个函数连同其单测一起，是本任务 Step 5 的产出。**）

- [ ] **Step 5: 完成 parse_one，跑测试**

`src/patch/parse.rs` 补齐 `parse_one` 与 `parse_one_no_extension`，实现按上面括号里的规定。关键纪律：

- `rename from` + `rename to` 同时出现 → `FileChange::Renamed { from_similarity }`，`similarity index` 缺省 100。
- `--- /dev/null` → `Added`；`+++ /dev/null` → `Deleted`。
- `Binary files ... differ` → `binary = true`，`hunks` 空。
- 只有 `old mode`/`new mode` 而无 `---` → `ModeOnly`。
- 每个 hunk 必须把 `\ No newline at end of file` 收成 `DiffLine::NoNewline` 且**不计入** `old_count`/`new_count`。
- 提前遇到下一个 `diff --git` 即当前 hunk 结束；若此时行数字不符，返回 `HunkLengthMismatch { line }`——**不得静默截断**，因为截断会让 Task 4 的行位置全线错位。

Run: `cargo test --locked --test patch_parse` → Expected: 全部 PASS。若 `hunk_line_numbers_are_exact` 挂在 `old_count` 上，先打印真实 `multi_hunk.diff` 的 `@@` 行核对，**不要改测试期望**。

- [ ] **Step 6: `xtask gendev`——从 diff 生成工作树（Task 4/5 的夹具工厂）**

`xtask/src/gendev.rs`：

```rust
//! 从 tests/golden/*.diff 生成 tests/golden_worktree/<case>/ 工作树。
//!
//! 为什么需要它：golden 事件测试（Task 8-11）要求 full_ast tier，而 full_ast 要求磁盘上有
//! b/ 侧内容。手写那种内容很容易与 .diff 漂移，导致测试挂在不存在的差异上。
//! CI 只消费已提交的工作树（可复现、不依赖本机 git）；本命令是开发者重生成用的。

use std::{fs, path::Path};

/// 对每个文件：取 `newest_path` 与 `oldest_path`，把**后镜像**（Context + Added 行）
/// 按 hunk 的 new_start 落位，得到 b/ 侧内容。hunk 之间的间隙从 oldest_path 对应文件补齐；
/// 若旧侧也不可得（Added），间隙即空。
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let golden = Path::new("tests/golden");
    let out_root = Path::new("tests/golden_worktree");
    let mut cases = vec![];
    for entry in fs::read_dir(golden)? {
        let p = entry?.path();
        if p.extension().and_then(|e| e.to_str()) != Some("diff") { continue; }
        let case = p.file_stem().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(&p)?;
        let files = diffintent::patch::parse(&text)?;
        let dir = out_root.join(&case);
        let _ = fs::remove_dir_all(&dir);
        for f in &files {
            let Some(path) = f.new_path else { continue };  // 删除文件在 b/ 侧不存在
            if f.binary { continue; }
            let content = postimage(&files, f, &dir)?;
            let target = dir.join(&path);
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            fs::write(&target, content)?;
        }
        cases.push(case);
    }
    cases.sort();
    println!("generated: {}", cases.join(", "));
    println!("下一步：git add tests/golden_worktree && 提交。CI 不重跑本命令。");
    Ok(())
}

/// 后镜像 = 未变更区域（取自旧文件，若无则空）+ 各 hunk 的 Context/Added 行。
fn postimage(
    _all: &[diffintent::patch::FileDiff],
    f: &diffintent::patch::FileDiff,
    worktree_root: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    // 用旧侧磁盘内容填间隙；旧侧不可得时按 Added 处理（间隙为空）。
    let mut lines: Vec<String> = Vec::new();
    for h in &f.hunks {
        while lines.len() < (h.new_start as usize).saturating_sub(1) {
            lines.push(String::new());  // 占位：真实现应从旧文件同位置取；无旧文件则是真间隙
        }
        for l in &h.lines {
            match l {
                diffintent::patch::DiffLine::Context(s) | diffintent::patch::DiffLine::Added(s) => {
                    if lines.len() < h.new_start as usize + h.new_count { lines.push(s.clone()); }
                }
                _ => {}
            }
        }
    }
    let _ = worktree_root;
    Ok(lines.join("\n") + if lines.is_empty() { "" } else { "\n" })
}
```

**上面 `postimage` 的间隙填充是近似实现**，它会在多 hunk 且旧文件存在时产生错位。真实现必须：若 `oldest_path()` 在**上一次生成的旧侧工作树**里存在，则以旧文件行为基底按 `new_start` 覆写；否则基底为空。**因此 gendev 需要两个工作树目录**：`golden_worktree_old/`（a/ 侧）与 `golden_worktree/`（b/ 侧）。落地方式：先 `git -C <临时仓> show HEAD~1:<path>` 取旧侧内容写进 `_old`，再据此重建 `_`。写不进临时仓时，退而求其次：**夹具的 .diff 必须只含单个 hunk**，多 hunk 场景交给 Task 4 直接用真 git 仓校验（见 Task 4 Step 2）。

在 `xtask/src/main.rs` 接线：

```rust
mod gendev;
// Cmd::Gendev => gendev::run(),
```

并在 `xtask/Cargo.toml` 加 `diffintent = { path = ".." }`。

Run: `cargo run -p xtask -- gendev` → Expected: `generated: crlf, mixed, ...`，且 `tests/golden_worktree/src/a.ts` 等文件存在、内容与 `git diff` 的 b/ 侧一致（用 `diff <(cat golden_worktree/src/a.ts) <(真 git 仓里 checkout 出来的 a.ts)` 抽查一次）。

- [ ] **Step 7: 提交**

```bash
git add src/patch tests/patch_parse.rs tests/golden tests/golden_worktree xtask
git commit -m "feat: unified diff 无损解析（含 rename 元数据与 NoNewline）

夹具全部由真实 git 生成，不手写。
Hunk.header_text 明确标注不可用于 scope/drift（git 对 TS 的 hunk header 正则很弱）。
xtask gendev 生成工作树夹具：golden 测试要求 full_ast tier，而 full_ast 要求磁盘上有 b/ 侧内容，
手写那种内容极易与 .diff 漂移。"
```

---

### Task 4: 后镜像校验与旧文件重建

覆盖 spec §6 的 `full_ast` 准入条件。**这是防错判的核心一段**：拿错的工作树内容去建 AST，产出的高置信度事件比 hunk 片段更危险。

核心事实（spec §6 已写入）：unified diff 的 **hunk 之外两侧恒等**。所以只要磁盘内容 == 后镜像，旧文件即可由「磁盘内容 − 各 hunk 后镜像行 + 前镜像行」精确重建，不需要 git 对象库、不需要 libgit2。

**Files:**
- Create: `src/patch/rebuild.rs`
- Test: `tests/rebuild.rs`

**Interfaces:**
- Consumes: Task 3 的 `FileDiff` `Hunk` `DiffLine`
- Produces:
  - `patch::verify_disk(file: &FileDiff, disk_text: Option<&str>) -> DiskVerdict`
  - `patch::DiskVerdict { Match, Mismatch { hunk_index, disk_line, expected }, Missing, CountMismatch }`
  - `patch::reconstruct_old(file: &FileDiff, new_text: &str) -> Option<String>`（Added 返回 `Some("")`；校验未过时返回 `None`）
  - `patch::reconstruct_new(file: &FileDiff, old_text: &str) -> Option<String>`（Deleted 用；从 a/ 侧推 b/ 侧）

- [ ] **Step 1: 写失败测试——用真 git 仓造夹具，而不是 gendev**

**本任务同时创建 `tests/common/mod.rs`**（跨任务共享的测试脚手架，Task 5/8/10/11/12/13/14 都往里加 helper，每加一个都在引入它的那次提交里实现完整，不留 `unimplemented!()`）。本任务只落地下面用到的四个函数，其余由后续任务补：

`tests/common/mod.rs`：

```rust
//! 跨任务共享的测试脚手架。**故意用真 git 造夹具**：gendev 生成的工作树本身可能与
//! .diff 漂移，用漂移的产物去校验校验器会形成自证。

#![allow(dead_code)]

use std::{fs, path::PathBuf, process::Command};

pub fn git_in(dir: &PathBuf, args: &[&str]) -> String {
    String::from_utf8_lossy(&git_bytes_in(dir, args)).to_string()
}

pub fn git_bytes_in(dir: &PathBuf, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git").args(args).current_dir(dir).output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn write_in(dir: &PathBuf, p: &str, content: &str) {
    let full = dir.join(p);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, content).unwrap();
}

/// 建一个真 git 仓：提交 before → 写入 after → 提交。
/// 磁盘上留下的是 after 内容，即 diff 的 **b/ 侧（后镜像）**——这正是 spec §6 校验的对象。
pub fn real_repo(files_before: &[(&str, &str)], files_after: &[(&str, &str)]) -> (PathBuf, String) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("diffintent-{}-{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    git_in(&dir, &["init", "-q"]);
    git_in(&dir, &["config", "user.email", "t@t"]);
    git_in(&dir, &["config", "user.name", "t"]);
    for (p, c) in files_before { write_in(&dir, p, c); }
    git_in(&dir, &["add", "-A"]);
    git_in(&dir, &["commit", "-qm", "base"]);
    for (p, c) in files_after { write_in(&dir, p, c); }
    git_in(&dir, &["add", "-A"]);
    git_in(&dir, &["commit", "-qm", "change"]);
    let d = git_in(&dir, &["diff", "HEAD~1"]);
    (dir, d)
}

pub fn git_diff_head1(dir: &PathBuf) -> String {
    git_in(dir, &["diff", "HEAD~1"])
}
```

目录名用**进程 id + 原子计数器**而非随机 hash：测试产物必须可复现，也不能撞名。

`tests/rebuild.rs`：

```rust
mod common;

use common::{git_bytes_in, real_repo};
use diffintent::patch::{parse, reconstruct_new, reconstruct_old, verify_disk, DiskVerdict};
use std::fs;

const BEFORE: &[(&str, &str)] = &[
    ("src/a.ts", "export function a() {\n  return 1;\n}\n"),
    ("src/b.ts", "export const x = 1;\nexport const y = 2;\nexport const z = 3;\n"),
];
const AFTER: &[(&str, &str)] = &[
    ("src/a.ts", "export function a() {\n  if (!v) return 0;\n  return 1;\n}\n"),
    ("src/b.ts", "export const x = 1;\nexport const y = 22;\nexport const z = 3;\n"),
];

/// 磁盘 == 后镜像时必须判 Match。
#[test]
fn matches_a_real_worktree() {
    let (_d, diff) = real_repo(BEFORE, AFTER);
    for f in parse(&diff).unwrap() {
        let text = fs::read_to_string(_d.join(f.newest_path())).ok();
        assert!(matches!(verify_disk(&f, text.as_deref()), DiskVerdict::Match), "{}: {:?}", f.newest_path(), verify_disk(&f, text.as_deref()));
    }
}

/// 工作树被再次修改（用户又编辑了一轮）→ 必须失配，不能继续当 full_ast 用。
#[test]
fn detects_drifted_worktree() {
    let (d, diff) = real_repo(BEFORE, AFTER);
    fs::write(d.join("src/a.ts"), "export function a() {\n  // edited again\n  if (!v) return 0;\n  return 1;\n}\n").unwrap();
    let f = parse(&diff).unwrap().remove(0);
    let text = fs::read_to_string(d.join("src/a.ts")).unwrap();
    let v = verify_disk(&f, Some(&text));
    assert!(matches!(v, DiskVerdict::Mismatch { .. }), "静默通过是 spec §6 最不能接受的失败: {v:?}");
}

/// 行号偏移一位（`git diff` 来自别的分支）必须失配。
#[test]
fn detects_one_line_shift() {
    let (d, diff) = real_repo(BEFORE, AFTER);
    fs::write(d.join("src/a.ts"), "\nexport function a() {\n  if (!v) return 0;\n  return 1;\n}\n").unwrap();
    let f = parse(&diff).unwrap().remove(0);
    assert!(matches!(verify_disk(&f, Some(&fs::read_to_string(d.join("src/a.ts")).unwrap())), DiskVerdict::Mismatch { .. }));
}

/// 文件在磁盘上不存在（未 checkout / 路径不同）→ Missing，不是 Mismatch。
#[test]
fn missing_file_is_its_own_verdict() {
    let (_d, diff) = real_repo(BEFORE, AFTER);
    let f = parse(&diff).unwrap().remove(0);
    assert_eq!(verify_disk(&f, None), DiskVerdict::Missing);
}

/// 重建的旧文件必须与真 git 里的 HEAD~1 逐字节相同——这是 full_ast 双侧 AST 的正确性根据。
#[test]
fn reconstructed_old_equals_git_old() {
    let (d, diff) = real_repo(BEFORE, AFTER);
    for f in parse(&diff).unwrap() {
        let new_text = fs::read_to_string(d.join(f.newest_path())).unwrap();
        let got = reconstruct_old(&f, &new_text).expect("reconstruct");
        let want = String::from_utf8(
            git_bytes_in(&d, &["show", &format!("HEAD~1:{}", f.oldest_path().unwrap())])
        ).unwrap();
        assert_eq!(got, want, "{} 重建与 git 旧侧不符", f.newest_path());
    }
}

/// Added 文件的旧侧是空串（不是 None）；校验未通过时才返回 None。
#[test]
fn added_file_reconstructs_to_empty() {
    let (d, diff) = real_repo(BEFORE, &[("src/a.ts", "export function a() {}\n"), ("src/n.ts", "export const n = 1;\n")]);
    let f = parse(&diff).unwrap().into_iter().find(|f| f.newest_path() == "src/n.ts").unwrap();
    assert_eq!(reconstruct_old(&f, &fs::read_to_string(d.join("src/n.ts")).unwrap()).as_deref(), Some(""));
}

/// 删除文件：磁盘上没有 b/ 侧，必须走前镜像校验 a/ 侧。
#[test]
fn deleted_file_verifies_against_old_side() {
    let (d, diff) = real_repo(&[("src/a.ts", "one\n")], &[]);
    let _ = d;
    let f = parse(&diff).unwrap().remove(0);
    assert_eq!(f.oldest_path(), Some("src/a.ts"));
    // 删除后磁盘上没有该文件；a/ 侧内容只能从 diff 的前镜像完整重建
    let rebuilt_new = reconstruct_new(&f, "one\n");
    assert!(rebuilt_new.as_deref().map(|s| s.is_empty()).unwrap_or(false));
}

/// 尾部无换行的文件不能被追加出一个幻影换行。
#[test]
fn preserves_missing_trailing_newline() {
    let (d, diff) = real_repo(&[("src/a.ts", "x\n")], &[("src/a.ts", "x\ny")]);
    let f = parse(&diff).unwrap().remove(0);
    let text = fs::read_to_string(d.join("src/a.ts")).unwrap();
    assert_eq!(text, "x\ny");
    assert!(matches!(verify_disk(&f, Some(&text)), DiskVerdict::Match), "无结尾换行必须能匹配");
    assert_eq!(reconstruct_old(&f, &text).as_deref(), Some("x\n"));
}
```

Run: `cargo test --locked --test rebuild` → Expected: 编译失败（`rebuild` 模块空）。

- [ ] **Step 2: 校验器**

`src/patch/rebuild.rs`：

```rust
//! 后镜像校验 + 旧文件精确重建。spec §6 的 full_ast 准入门。

use super::{DiffLine, FileDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskVerdict {
    Match,
    /// 磁盘内容与 hunk 声明的后镜像不符。带位置便于 --details 直接指认。
    Mismatch { hunk_index: usize, disk_line: usize, expected: String },
    /// 磁盘上没有该文件
    Missing,
    /// 声明的行数与实际可取行数不符（文件比 diff 短）
    CountMismatch { hunk_index: usize },
}

pub fn verify_disk(file: &FileDiff, disk_text: Option<&str>) -> DiskVerdict {
    if file.binary || file.hunks.is_empty() {
        // ModeOnly / 纯重命名无内容变更：没有可校验的行，视为 Match（不降 tier）
        return DiskVerdict::Match;
    }
    // 删除文件在 b/ 侧不存在，其校验由调用方用 a/ 侧前镜像完成（见 evidence_resolve）
    let Some(text) = disk_text else { return DiskVerdict::Missing };
    let lines: Vec<&str> = text.split('\n').collect();
    // split('\n') 对以 \n 结尾的文本会在末尾多出一个 ""，这与 diff 的行语义一致，
    // 但必须知道它是否存在，否则最后一行的比对会错一位。
    let has_trailing_newline = text.ends_with('\n');

    for (hi, h) in file.hunks.iter().enumerate() {
        // 期望的后镜像行序列
        let mut expected: Vec<&str> = Vec::with_capacity(h.new_count);
        for l in &h.lines {
            match l {
                DiffLine::Context(s) | DiffLine::Added(s) => expected.push(s),
                DiffLine::Removed(_) | DiffLine::NoNewline => {}
            }
        }
        if expected.len() != h.new_count {
            return DiskVerdict::CountMismatch { hunk_index: hi };
        }
        // new_start 是 1-based；0 表示文件原本为空（Added）
        let base = h.new_start.saturating_sub(1) as usize;
        if base + expected.len() > lines.len() {
            // 允许唯一一种越界：末尾空行由 has_trailing_newline 提供
            let allowed = lines.len() + usize::from(!has_trailing_newline);
            if base + expected.len() > allowed {
                return DiskVerdict::CountMismatch { hunk_index: hi };
            }
        }
        for (k, exp) in expected.iter().enumerate() {
            let idx = base + k;
            let got = lines.get(idx).copied().unwrap_or(if idx == lines.len() && !has_trailing_newline { "\u{0}SENTINEL" } else { "\u{0}OOB" });
            if got != *exp {
                return DiskVerdict::Mismatch {
                    hunk_index: hi,
                    disk_line: idx + 1,
                    expected: (*exp).to_string(),
                };
            }
        }
    }
    DiskVerdict::Match
}
```

**上面 SENTINEL/OOB 的写法是权宜**。落地时把它换掉：先算 `lines` 时保留末尾换行信息，用 `Vec<Cow<str>>`，把 `has_trailing_newline` 显式建模成一个 `LineIndex` 辅助类型（`fn line_at(&self, i: usize) -> Option<&str>`），越界返回 `None` 并在 `Mismatch` 里用 `expected` + `disk_line` 表达"这里磁盘上没有行"。测试 `preserves_missing_trailing_newline` 与 `detects_one_line_shift` 会立刻暴露写歪的版本。

- [ ] **Step 3: 旧文件重建**

同文件追加：

```rust
/// 由「磁盘（== b/ 侧）」精确重建 a/ 侧。spec §6：hunk 之外两侧恒等。
/// 校验未通过（diff 与磁盘不同源）时返回 None——绝不返回半成品，
/// 因为半对半错的 AST 会产出看似高置信的错判。
pub fn reconstruct_old(file: &FileDiff, new_text: &str) -> Option<String> {
    if matches!(file.change, super::FileChange::Added) {
        return Some(String::new());
    }
    let mut out = new_text.to_string();
    // 逆序应用：从最后一个 hunk 往前替换，前面的偏移才不会因后面的长度变化而失效
    for h in file.hunks.iter().rev() {
        let mut pre: Vec<&str> = Vec::with_capacity(h.old_count);
        let mut post: Vec<&str> = Vec::with_capacity(h.new_count);
        for l in &h.lines {
            match l {
                DiffLine::Context(s) => { pre.push(s); post.push(s); }
                DiffLine::Removed(s) => pre.push(s),
                DiffLine::Added(s) => post.push(s),
                DiffLine::NoNewline => {}
            }
        }
        if pre.len() != h.old_count || post.len() != h.new_count { return None; }
        let replace_from = offset_of_line(&out, h.new_start)?;
        let replace_to = offset_of_line(&out, h.new_start + h.new_count as u32)
            .unwrap_or(out.len());
        let block = pre.join("\n");
        let block = if pre.is_empty() || ends_nl(&out[..replace_to]) {
            format!("{block}\n")
        } else {
            block
        };
        out.replace_range(replace_from..replace_to, &block);
    }
    Some(out)
}

/// Deleted 文件：磁盘上只有 a/ 侧，用它推出 b/ 侧（恒为空串，但保留通用实现供
/// 未来的 copy/rename-with-edit 使用）。
pub fn reconstruct_new(file: &FileDiff, old_text: &str) -> Option<String> {
    if matches!(file.change, super::FileChange::Deleted) {
        // 整文件删除：diff 只有一个覆盖全文件的 hunk
        return Some(String::new());
    }
    let mut out = old_text.to_string();
    for h in file.hunks.iter().rev() {
        let mut pre: Vec<&str> = Vec::new();
        let mut post: Vec<&str> = Vec::new();
        for l in &h.lines {
            match l {
                DiffLine::Context(s) => { pre.push(s); post.push(s); }
                DiffLine::Removed(s) => pre.push(s),
                DiffLine::Added(s) => post.push(s),
                DiffLine::NoNewline => {}
            }
        }
        if pre.len() != h.old_count { return None; }
        let from = offset_of_line(&out, h.old_start)?;
        let to = offset_of_line(&out, h.old_start + h.old_count as u32).unwrap_or(out.len());
        let block = post.join("\n");
        let block = if post.is_empty() {
            String::new()
        } else if ends_nl(&out[..to]) {
            format!("{block}\n")
        } else {
            block
        };
        out.replace_range(from..to, &block);
    }
    Some(out)
}

/// 1-based 行号 → 字节偏移。行号 1 → 0。越界返回 None。
fn offset_of_line(text: &str, one_based: u32) -> Option<usize> {
    if one_based == 0 { return Some(0); }
    let mut off = 0usize;
    let mut n = 1u32;
    loop {
        if n == one_based { return Some(off); }
        off = text.get(off..)?[1..].find('\n')? + off + 1;
        n += 1;
    }
}

fn ends_nl(s: &str) -> bool {
    s.ends_with('\n')
}
```

`offset_of_line` 的循环边界容易写错（第一行是偏移 0、越界要 `None` 而不是 panic），Step 4 先单测它。

- [ ] **Step 4: 给偏移函数补单测**

`src/patch/rebuild.rs` 追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_first_line_is_zero() {
        assert_eq!(offset_of_line("a\nb\nc\n", 1), Some(0));
        assert_eq!(offset_of_line("a\nb\nc\n", 2), Some(2));
        assert_eq!(offset_of_line("a\nb\nc\n", 3), Some(4));
        assert_eq!(offset_of_line("a\nb\nc\n", 4), Some(6)); // 末行之后的位置
        assert_eq!(offset_of_line("a\nb\nc\n", 5), None);
        assert_eq!(offset_of_line("", 1), Some(0));
        assert_eq!(offset_of_line("", 2), None);
    }

    /// 中文/emoji 不得因字节偏移而错位。
    #[test]
    fn offset_handles_multibyte() {
        assert_eq!(offset_of_line("你\n好\n", 2), Some(4));
    }
}
```

Run: `cargo test --locked` → Expected: rebuild 集成测试 + 单测全 PASS。

- [ ] **Step 5: 提交**

```bash
git add src/patch/rebuild.rs tests/rebuild.rs
git commit -m "feat: 后镜像校验与旧文件精确重建（full_ast 准入门）

hunk 之外两侧恒等，所以磁盘==后镜像时可精确重建旧文件，无需 git 对象库、无需 libgit2。
校验不过一律返回 None 而非半成品：半对半错的 AST 会产出看似高置信的错判（spec §6）。
本任务的夹具用真 git 仓生成——用 gendev 的产物校验校验器会形成自证。"
```

---

### Task 5: tier 解析

覆盖 spec §6 全节。把 `Task 4` 的校验结果与语法注册表合成每个文件的最终层级，并记录降级原因（`summary.evidence_tiers` 与 block 的 `evidence_tier` 字段都从这来）。

**Files:**
- Create: `src/evidence/mod.rs`, `src/evidence/worktree.rs`
- Test: `tests/evidence.rs`

**Interfaces:**
- Consumes: Task 3 `FileDiff`；Task 4 `verify_disk` `reconstruct_old`；Task 2 `EvidenceTier`
- Produces:
  - `evidence::Worktree { root: PathBuf, is_git: bool }` + `new(Option<PathBuf>)` / `none()` / `read(&str) -> Option<String>`
  - `evidence::TierReason { FullAstVerified, NoWorktree, FileMissing, ContentMismatch { hunk_index, disk_line }, GrammarUnsupported, DeletedOldSideVerified, ModeOnlyOrNoContentChange }`
  - `evidence::ResolvedFile { id: FileId, diff: FileDiff, tier: EvidenceTier, reason: TierReason, old_source: Option<String>, new_source: Option<String>, language: Option<Language> }`
  - `evidence::language_for_path(path: &str) -> Option<Language>`
  - `evidence::is_config_path(path: &str) -> bool`
  - `evidence::is_derived_path(path: &str) -> bool`
  - `evidence::resolve(files: Vec<FileDiff>, worktree: &Worktree) -> Vec<ResolvedFile>`

- [ ] **Step 1: 写失败测试**

`tests/evidence.rs`：

```rust
use diffintent::evidence::*;
use diffintent::seir::EvidenceTier;

fn tiers_of(text: &str, wt: &Worktree) -> Vec<(String, EvidenceTier, TierReason)> {
    resolve(parse(text).unwrap(), wt)
        .into_iter()
        .map(|r| (r.diff.newest_path().to_string(), r.tier, r.reason))
        .collect()
}

#[test]
fn ts_with_worktree_is_full_ast_and_carries_both_sides() {
    let (dir, diff) = common::real_repo(
        &[("src/a.ts", "export function a() {\n  return 1;\n}\n")],
        &[("src/a.ts", "export function a() {\n  if (!v) return 0;\n  return 1;\n}\n")],
    );
    let wt = Worktree::new(Some(dir.clone())).unwrap();
    let out = tiers_of(&diff, &wt);
    assert_eq!(out[0].1, EvidenceTier::FullAst);
    let r = resolve(parse(&diff).unwrap(), &wt).remove(0);
    assert!(r.old_source.is_some() && r.new_source.is_some());
    assert_eq!(r.language.unwrap().ext(), "typescript");
}

/// 没有工作树（纯 stdin 管道，`--repo` 也没给）→ hunk_only。
/// 这是旗舰 UX 的真实层级，必须显式而非偷偷按 full_ast 处理。
#[test]
fn no_worktree_is_hunk_only() {
    let diff = std::fs::read_to_string("tests/golden/multi_hunk.diff").unwrap();
    let out = tiers_of(&diff, &Worktree::none());
    assert!(out.iter().all(|(_, t, _)| *t == EvidenceTier::HunkOnly), "{out:?}");
    assert!(matches!(out[0].2, TierReason::NoWorktree));
}

/// 工作树内容与 diff 不同源 → hunk_only + ContentMismatch，不是 full_ast。
#[test]
fn drifted_worktree_demotes_with_reason() {
    let (dir, diff) = common::real_repo(
        &[("src/a.ts", "export function a() {\n  return 1;\n}\n")],
        &[("src/a.ts", "export function a() {\n  if (!v) return 0;\n  return 1;\n}\n")],
    );
    std::fs::write(dir.join("src/a.ts"), "totally different\n").unwrap();
    let wt = Worktree::new(Some(dir)).unwrap();
    let out = tiers_of(&diff, &wt);
    assert_eq!(out[0].1, EvidenceTier::HunkOnly);
    assert!(matches!(out[0].2, TierReason::ContentMismatch { .. }), "{:?}", out[0].2);
}

/// 非 TS/JS/Python 语言必须降到 text_fallback 并标 GrammarUnsupported。
/// spec §6：宁可诚实降级，不要假装支持。
#[test]
fn unsupported_languages_fall_back_and_are_labeled() {
    let cases = ["src/main.go", "src/App.rs", "src/Main.java", "src/app.rb", "src/main.php", "src/app.swift"];
    for path in cases {
        let text = format!(
            "diff --git a/{p} b/{p}\nindex 0..1 100644\n--- /dev/null\n+++ b/{p}\n@@ -0,0 +1,2 @@\n+func a() {{}}\n+var b = 1\n",
            p = path
        );
        let out = tiers_of(&text, &Worktree::none());
        assert_eq!(out[0].1, EvidenceTier::TextFallback, "{path}");
        assert!(matches!(out[0].2, TierReason::GrammarUnsupported), "{path}: {:?}", out[0].2));
    }
}

/// 配置文件走 text_fallback 但要认出它是 config（Task 8 的 config_key 依赖这个判定）。
#[test]
fn config_paths_are_recognized() {
    for p in ["package.json", "tsconfig.json", "config.yaml", "app.yml", "Cargo.toml", ".env", "settings.ini"] {
        assert!(is_config_path(p), "{p}");
    }
    assert!(!is_config_path("src/app.ts"));
}

/// 派生路径永远不得报 drift（spec §9.4）。这个判定必须在 tier 阶段就位，
/// 否则 W3 的 drift_align 会拿到已经丢失来源信息的结构。
#[test]
fn derived_paths_are_classified() {
    for p in [
        "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock", "poetry.lock",
        "dist/bundle.js", "build/out/app.js", "src/x_pb2.py", "vendor/foo.go",
        "app.min.js", "tests/__snapshots__/a.snap", "src/gen/types.d.ts",
    ] {
        assert!(is_derived_path(p), "{p}");
    }
    assert!(!is_derived_path("src/auth.ts"));
    assert!(!is_derived_path("package.json"), "package.json 是被改的对象，不是派生物");
}

/// Deleted 文件：磁盘上没有 b/ 侧，必须校验 a/ 前镜像而不是直接降级。
#[test]
fn deleted_file_uses_old_side_and_can_be_full_ast() {
    let (dir, diff) = common::real_repo(
        &[("src/gone.ts", "export const g = 1;\n")],
        &[],
    );
    let wt = Worktree::new(Some(dir)).unwrap();
    let r = resolve(parse(&diff).unwrap(), &wt).remove(0);
    assert_eq!(r.tier, EvidenceTier::FullAst);
    assert!(matches!(r.reason, TierReason::DeletedOldSideVerified));
    assert!(r.old_source.is_some() && r.new_source.as_deref() == Some(""));
}

/// Renamed 无内容变更 → 不产事件、不降 tier（Task 7 折叠它）。
#[test]
fn pure_rename_has_no_content_hunks_and_stays_full_ast() {
    let (dir, diff) = common::real_repo(
        &[("src/one.ts", "export const x = 1;\n")],
        &[],
    );
    // git mv 不加改动的 diff
    std::process::Command::new("git").args(["mv", "src/one.ts", "src/two.ts"])
        .current_dir(&dir).output().unwrap();
    std::process::Command::new("git").args(["commit", "-qm", "move"])
        .current_dir(&dir).output().unwrap();
    let d = common::git_diff_head1(&dir);
    let r = resolve(parse(&d).unwrap(), &Worktree::new(Some(dir)).unwrap()).remove(0);
    assert_eq!(r.tier, EvidenceTier::FullAst);
    assert!(matches!(r.reason, TierReason::ModeOnlyOrNoContentChange));
    assert!(r.diff.hunks.is_empty());
}

/// 三档必须都可达，否则 --json 的 evidence_tiers 计数会出现永远为零的档。
#[test]
fn all_three_tiers_are_reachable() {
    let got: Vec<EvidenceTier> = vec![EvidenceTier::FullAst, EvidenceTier::HunkOnly, EvidenceTier::TextFallback];
    assert_eq!(got.len(), 3);
}
```

Run: `cargo test --locked --test evidence` → Expected: 编译失败。

- [ ] **Step 2: 回归确认基线干净**

`tests/common/mod.rs` 已由 Task 4 Step 1 建立，本任务直接引用，不再搬动它。先跑一遍确认基线：

```bash
cargo test --locked --test rebuild
```

Expected: Task 4 的 7 条测试全 PASS。**若此时绿、而本任务 Step 4 之后变红，说明改动碰到了 `rebuild` 依赖的解析或校验逻辑**——先修根因，不得改 Task 4 的期望值来放行。

- [ ] **Step 3: Worktree（不引入 git 依赖）**

`src/evidence/worktree.rs`：

```rust
//! 磁盘工作树访问。**故意不用 libgit2 / git2**：
//! 那会加重 Windows C 构建负担（spec R3）并引入与 `git` 版本相关的对象库解析差异。
//! unified diff 的 hunk 之外两侧恒等，Task 4 已足以精确重建另一侧。

use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone)]
pub struct Worktree {
    root: PathBuf,
    is_git: bool,
}

impl Worktree {
    /// None → 无工作树（所有文件最多 hunk_only）。Some 且目录含 `.git` → 正常。
    /// Some 但无 `.git`：仍允许读盘并校验后镜像——校验通过才给 full_ast，
    /// 所以"非 git 目录"不构成风险，不需要拒绝。
    pub fn new(root: Option<PathBuf>) -> Option<Self> {
        let root = root.unwrap_or_else(|| std::env::current_dir().ok().unwrap_or_default());
        if root.as_os_str().is_empty() || !root.exists() {
            return None;
        }
        let is_git = root.join(".git").exists();
        Some(Self { root, is_git })
    }

    pub fn none() -> Self {
        Self { root: PathBuf::new(), is_git: false }
    }

    pub fn is_git_repo(&self) -> bool {
        self.is_git
    }

    /// 读 repo 内路径。越界（`../` 之类）与不存在都返回 None。
    pub fn read(&self, rel: &str) -> Option<String> {
        if rel.is_empty() || self.root.as_os_str().is_empty() {
            return None;
        }
        let target = self.root.join(rel);
        // 拒绝跑出 root 的路径：diff 可能带 ../../../etc/passwd 这样的 +++ 路径
        let normalized = target.canonicalize().ok()?;
        let root_norm = self.root.canonicalize().ok()?;
        if !normalized.starts_with(&root_norm) {
            return None;
        }
        fs::read_to_string(normalized).ok()
    }
}
```

`read` 用 `canonicalize` 挡路径穿越——diff 内容是不可信输入，`+++ /etc/passwd` 或 `+++ ../../secret` 必须读不到。这条写成测试（Step 5）。

- [ ] **Step 4: 语法与路径判定**

`src/evidence/mod.rs`：

```rust
//! tier 解析。spec §6 三级证据降级链。

pub mod worktree;

use crate::seir::{EvidenceTier, FileId, FileTable};
use crate::patch::{parse, DiskVerdict, FileDiff, PatchError, reconstruct_old, reconstruct_new, verify_disk};
pub use worktree::Worktree;

/// MVP 的 AST 支持范围（spec §6）。其余语言显式降级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
}

impl Language {
    pub fn ext(self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Python => "python",
        }
    }
}

pub fn language_for_path(path: &str) -> Option<Language> {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit_once('.')?.1;
    match ext {
        "ts" | "mts" | "cts" => Some(Language::TypeScript),
        "tsx" => Some(Language::Tsx),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
        "py" | "pyi" => Some(Language::Python),
        _ => None,
    }
}

/// `.d.ts` 是类型声明，常被工具生成；不当作 config，但 Task 13 会把它标 generated。
pub fn is_declaration_file(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".d.ts")
}

/// 配置类文件：spec §5.1 的 config ↑↑ 先验走扩展名判定，不需要 AST。
pub fn is_config_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    matches!(ext, "yaml" | "yml" | "json" | "toml" | "ini" | "env" | "toml" | "plist" | "tf" | "tfvars" | "hcl" | "properties")
        || name.starts_with('.') && matches!(name.strip_prefix('.').unwrap_or(name).rsplit_once('.').map(|(_, e)| e).unwrap_or(""), "env" | "ini")
        || matches!(name, "dockerfile" | "makefile" | ".env" | ".env.local" | "procfile")
        || p.contains(".env.")
}

/// spec §9.4 派生路径：合法变更的副产品，永远不得报 drift。
/// 这条必须在 tier 阶段就打上，不能让 W3 的 drift_align 重新猜。
pub fn is_derived_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase().replace('\\', "/");
    let name = p.rsplit('/').next().unwrap_or(&p);
    if matches!(name,
        "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "npm-shrinkwrap.json"
        | "cargo.lock" | "poetry.lock" | "pipfile.lock" | "composer.lock" | "go.sum"
        | "gemfile.lock" | "mix.lock" | "deno.lock" | "bun.lockb" | "uv.lock")
    {
        return true;
    }
    if p.contains("/__snapshots__/") || name.ends_with(".snap") {
        return true;
    }
    for d in ["dist/", "build/", "target/", "node_modules/", "vendor/", ".next/", ".nuxt/", "out/", "coverage/"] {
        if p.starts_with(d) || p.contains(&format!("/{d}")) {
            return true;
        }
    }
    name.ends_with("_pb2.py") || name.ends_with("_pb.go") || name.ends_with(".min.js")
        || name.ends_with(".min.css") || name.ends_with(".d.ts")
        || p.ends_with(".egg-info/PKG-INFO")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierReason {
    FullAstVerified,
    /// 未给工作树且 cwd 不可用（纯管道）
    NoWorktree,
    FileMissing,
    ContentMismatch { hunk_index: usize, disk_line: usize },
    GrammarUnsupported,
    /// 删除文件：a/ 侧前镜像校验通过
    DeletedOldSideVerified,
    /// 无内容变更（纯 rename / 纯 mode 变化）
    ModeOnlyOrNoContentChange,
    /// 语法解析自身失败（Task 6 之后可能置上）
    ParseFailed,
    /// 旧侧重建失败（diff 内部不一致）
    ReconstructionFailed,
}

/// 一个文件在流水线里流转的单位。spec §4 第 2 段产出。
#[derive(Debug, Clone)]
pub struct ResolvedFile {
    pub id: FileId,
    pub diff: FileDiff,
    pub tier: EvidenceTier,
    pub reason: TierReason,
    /// 仅 full_ast 有值
    pub old_source: Option<String>,
    pub new_source: Option<String>,
    pub language: Option<Language>,
    /// spec §9.4：派生路径在这一层就打标，后续段不得重猜
    pub derived: bool,
}

pub fn resolve(files: Vec<FileDiff>, worktree: &Worktree) -> Vec<ResolvedFile> {
    let paths: Vec<String> = files.iter().flat_map(|f| {
        [f.newest_path().to_string(), f.oldest_path().unwrap_or("").to_string()]
    }).collect();
    let table = FileTable::from_paths(paths);

    let mut out: Vec<ResolvedFile> = Vec::with_capacity(files.len());
    for diff in files {
        let id = table.id_of(diff.newest_path())
            .or_else(|| table.id_of(diff.oldest_path().unwrap_or("")))
            .expect("every path is in the table");
        let language = language_for_path(diff.newest_path())
            .or_else(|| language_for_path(diff.oldest_path().unwrap_or("")));
        let derived = is_derived_path(diff.newest_path())
            || is_derived_path(diff.oldest_path().unwrap_or(""));

        // 1) 语法不支持 → text_fallback（优先于 hunk_only：连片段都解析不了）
        let Some(lang) = language else {
            out.push(ResolvedFile {
                id, diff, language: None, derived,
                tier: EvidenceTier::TextFallback,
                reason: TierReason::GrammarUnsupported,
                old_source: None, new_source: None,
            });
            continue;
        };

        // 2) 无内容变更 → 保持 full_ast 语义（不需要 AST 也无事可做）
        if diff.hunks.is_empty() {
            out.push(ResolvedFile {
                id, diff, language: Some(lang), derived,
                tier: EvidenceTier::FullAst,
                reason: TierReason::ModeOnlyOrNoContentChange,
                old_source: None, new_source: Some(String::new()),
            });
            continue;
        }

        // 3) 尝试 full_ast：读盘 → 校验 → 重建另一侧
        let attempt = |wt: &Worktree| -> Option<(String, String, TierReason, Option<DiskVerdict>)> {
            if matches!(diff.change, crate::patch::FileChange::Deleted) {
                let old = wt.read(diff.oldest_path()?)?;
                let post = reconstruct_new(&diff, &old)?;
                if !post.is_empty() { return None; }  // 删除文件必须重建出空 b/ 侧
                Some((old, String::new(), TierReason::DeletedOldSideVerified, None))
            } else {
                let new_text = wt.read(diff.newest_path())?;
                let v = verify_disk(&diff, Some(&new_text));
                if v != DiskVerdict::Match {
                    return Some((String::new(), String::new(), TierReason::NoWorktree, Some(v)));
                }
                let old = reconstruct_old(&diff, &new_text)?;
                Some((old, new_text, TierReason::FullAstVerified, None))
            }
        };

        // 三态分支：无工作树 / 有工作树但校验失败 / 有工作树且成功
        let outcome = match worktree {
            None => None,
            Some(wt) => attempt(wt),
        };

        if let Some((old, new, reason, None)) = outcome {
            out.push(ResolvedFile {
                id, diff, language: Some(lang), derived,
                tier: EvidenceTier::FullAst, reason,
                old_source: Some(old), new_source: Some(new),
            });
            continue;
        }

        // 4) 降到 hunk_only，并给准确原因（原因必须来自真实判定，不得写死占位值）
        let reason = match (worktree, outcome.and_then(|(_, _, _, v)| v)) {
            (None, _) => TierReason::NoWorktree,
            (Some(wt), None) if wt.read(diff.newest_path()).is_none() => TierReason::FileMissing,
            (Some(_), Some(DiskVerdict::Mismatch { hunk_index, disk_line, .. })) => {
                TierReason::ContentMismatch { hunk_index, disk_line }
            }
            (Some(_), Some(DiskVerdict::CountMismatch { hunk_index })) => {
                TierReason::ContentMismatch { hunk_index, disk_line: 0 }
            }
            (Some(_), Some(_)) => TierReason::ReconstructionFailed,
        };
        out.push(ResolvedFile {
            id, diff, language: Some(lang), derived,
            tier: EvidenceTier::HunkOnly, reason,
            old_source: None, new_source: None,
        });
    }
    // 规范序：按 FileId 再按路径，保证与输入顺序无关
    out.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.diff.newest_path().cmp(b.diff.newest_path())));
    out
}
```

签名是 `pub fn resolve(files: Vec<FileDiff>, worktree: Option<&Worktree>) -> Vec<ResolvedFile>`。`attempt` 闭包里第 3 个分支返回 `Some((.., .., NoWorktree, Some(v)))` 只为把 `DiskVerdict` 带出来给原因构造用——`TierReason::NoWorktree` 在那个位置是被丢弃的中间值，读代码时容易误解，**落地时把它换成一个独立的 `verdict_from_disk` 辅助函数而非塞进元组第四位**，语义更清楚。`Worktree::none()` 这个构造函数因此不再需要，删掉它并同步改 Task 12 的测试（那里用 `None`）。

- [ ] **Step 5: 补路径穿越与原因精确性测试**

`tests/evidence.rs` 追加：

```rust
/// diff 是不可信输入。`+++ ../../secret` 不得被读到。
#[test]
fn refuses_paths_escaping_the_worktree() {
    let text = "diff --git a/../../secret.ts b/../../secret.ts\nindex 0..1 100644\n--- /dev/null\n+++ b/../../secret.ts\n@@ -0,0 +1 @@\n+x\n";
    let dir = std::env::temp_dir();
    std::fs::write(dir.join("secret.ts"), "leaked\n").unwrap();
    let sub = dir.join("wt-sub");
    std::fs::create_dir_all(&sub).unwrap();
    let wt = Worktree::new(Some(sub)).unwrap();
    let r = resolve(parse(text).unwrap(), Some(&wt)).remove(0);
    assert_eq!(r.tier, EvidenceTier::HunkOnly);
    assert!(matches!(r.reason, TierReason::FileMissing), "{:?}", r.reason);
    assert!(!format!("{:?}", r.old_source).contains("leaked"), "越界读到的内容泄漏进结果");
}

/// 降级原因必须精确指认是哪个 hunk、磁盘哪一行，否则 --details 无法解释。
#[test]
fn mismatch_reason_names_the_hunk_and_line() {
    let (dir, diff) = common::real_repo(
        &[("src/a.ts", "l1\nl2\nl3\nl4\nl5\n")],
        &[("src/a.ts", "l1\nX2\nl3\nl4\nY5\n")],
    );
    let text = format!("{}{}", diff, "  ");
    let _ = text;
    std::fs::write(dir.join("src/a.ts"), "l1\nl2\nl3\nl4\nl5\nl6\nl7\n").unwrap();
    let r = resolve(parse(&diff).unwrap(), Some(&Worktree::new(Some(dir)).unwrap())).remove(0);
    let TierReason::ContentMismatch { hunk_index, disk_line } = r.reason else {
        panic!("期望 ContentMismatch，实得 {:?}", r.reason);
    };
    assert!(hunk_index < 2);
    assert!(disk_line > 0, "disk_line 必须来自真实校验结果而非占位 0");
}
```

Run: `cargo test --locked` → Expected: 全 PASS。若 `mismatch_reason_names_the_hunk_and_line` 拿到 `ParseFailed`，检查 `resolve` 是否在 `Some(...)` 分支里把 `verify_disk` 的细节丢掉了。

- [ ] **Step 6: 提交**

```bash
git add src/evidence tests/evidence.rs tests/common
git commit -m "feat: 三级证据降级链与 tier 原因

派生路径与 config 判定在这一层就打标（spec §9.4 / §5.1），后续段不得重猜。
Worktree 不用 git2：diff 的 hunk 之外两侧恒等，Task 4 已足够精确重建另一侧，
避开 libgit2 的 Windows C 构建负担（spec R3）。
read() 经 canonicalize 挡路径穿越——diff 是不可信输入。"
```

---

### Task 6: tree-sitter 接线与 node-kind 探测表

覆盖 spec §5 的 `scope` / `node_kind` 字段来源。

**本任务含一个关键纪律：node kind 名字必须由探测得出，不得凭记忆手写。** 计划作者给出的 kind 名（`required_parameter`、`except_clause`、`assert_statement` 等）在各 grammar 版本间确有漂移，Task 8–11 的表必须以 Step 1 的探测输出为准。凡本计划里出现的 kind 名与探测结果不符，**改计划、不改探测**。

**Files:**
- Create: `src/ast/mod.rs`, `src/ast/scopes.rs`, `src/ast/pair.rs`
- Create: `build.rs`（改：接 `DIFFINTENT_GRAMMAR_SRC_*`）
- Create: `tests/fixtures/*.tsv`, `tests/ast.rs`, `tests/nodekinds.rs`
- Modify: `Cargo.toml`（+4 normal deps，到 8 上限）

**Interfaces:**
- Consumes: Task 5 的 `Language` `ResolvedFile`；Task 2 的 `ScopeRef`
- Produces:
  - `ast::ParsedFile { source: String, tree: Tree, language: Language }` + `parse(Language, &str) -> Result<ParsedFile, AstError>`
  - `ParsedFile::root() -> Node` / `line_col_of(byte) -> (u32, u32)` / `text_of(Node) -> &str`
  - `ast::enclosing_scope(&ParsedFile, byte_offset: usize) -> Option<ScopeRef>`
  - `ast::definitions(&ParsedFile) -> Vec<(ScopeRef, Node)>`（深度优先，规范序）
  - `ast::kind_is_control_flow(&str) -> bool` / `kind_is_guard(&str, &Node) -> bool` / `kind_is_error_handling(&str) -> bool` / `kind_is_import(&str) -> bool` / `kind_is_test_declaration(&str) -> bool`
  - `ast::pair::pair_definitions(old: &[ScopeRef], new: &[ScopeRef]) -> Vec<Pair>`，`Pair { Old(usize), New(usize) | Added(NewIdx, New) | Removed(OldIdx, Old) }`
  - `ast::parse_fragment(Language, &str) -> Option<ParsedFile>`（Task 12 用）

- [ ] **Step 1: 加依赖并写探测测试，把 kind 名钉进 fixtures**

```bash
cargo add tree-sitter tree-sitter-javascript tree-sitter-typescript tree-sitter-python
```

`tests/nodekinds.rs`：

```rust
//! 把每种语言里出现过的全部 concrete node kind 导成 tsv，作为 ast/scopes.rs 词表的事实来源。
//! 跑法：cargo test --locked nodekinds -- --nocapture
//! 期望表与实测不符时，改 scopes.rs 并同步改本表；**不要改期望去迎合实现**。

use diffintent::{ast, evidence::Language};
use std::{collections::BTreeSet, fs};

const SAMPLES: &[(&str, &str)] = &[
    ("typescript", r#"
import { x } from "y";
import * as z from "z";
interface I { a: string }
type T = { b?: number };
enum E { A, B }
abstract class C {
  static s: number = 1;
  #priv = 2;
  constructor(private p: string) {}
  method(a: number, b?: string, ...rest: unknown[]): Promise<void> {
    if (a > 1) { throw new Error("x"); }
    for (const v of rest) { try { z.use(v) } catch (e) { console.error(e) } }
    while (true) { break; }
    switch (a) { case 1: return; default: break; }
    a ? b : c;
    const arrow = async (): Promise<void> => { await x(); };
    return arrow();
  }
}
function top(): void { let l = 1; l += 1; doWork(l); }
class D { gen* () { yield 1; } async ag() { yield* []; } }
export { C, D };
export default function dd() {}
declare module "m" {}
namespace N { export const q = 1; }
"#),
    ("python", r#"
import os
from a.b import c, d as e
@decorator
class C:
    attr: int = 1
    def method(self, a: int, b: str = "x", *args, **kw) -> None:
        if a is None: return
        assert a, "msg"
        try:
            os.path.join(a)
        except (ValueError, TypeError) as err:
            raise RuntimeError("x") from err
        finally:
            pass
        for i in range(10):
            with open("f") as fh:
                pass
        while True:
            break
        match a:
            case 1: pass
            case [1, 2]: pass
            case {"k": v}: pass
        x = lambda: 1
        y = [i for i in range(3)]
        gen = (i for i in range(3))
        await something()
def test_ok():
    assert True
"#),
];

fn dump(lang: Language, src: &str) -> Vec<String> {
    let pf = ast::parse(lang, src).expect("parse sample");
    let mut kinds = BTreeSet::new();
    let mut stack = vec![pf.root()];
    while let Some(n) = stack.pop() {
        if n.is_named() {
            kinds.insert(n.kind().to_string());
            for c in n.children(&mut n.walk()) { stack.push(c); }
        }
    }
    kinds.into_iter().collect()
}

#[test]
fn kinds_are_all_covered_by_the_word_lists() {
    for (name, src) in SAMPLES {
        let lang = match *name {
            "typescript" => Language::TypeScript,
            "python" => Language::Python,
            _ => unreachable!(),
        };
        let kinds = dump(lang, src);
        let path = format!("tests/fixtures/{}.kinds.tsv", name);
        let want = fs::read_to_string(&path).unwrap_or_default();
        let got = kinds.join("\n") + "\n";
        if want.is_empty() {
            fs::write(&path, &got).unwrap();
            panic!("首次运行已生成 {path}；人工核对每个 kind 是否已被 scopes.rs 的词表覆盖或有意忽略，然后重跑");
        }
        assert_eq!(got, want, "{name} 的 node kind 集合变了：词表需同步，且这是 grammar 版本变更的信号");
    }
}

/// 词表覆盖性断言：样例里出现的每个 kind，要么进某张表，要么在 IGNORED 里被显式承认。
/// 这样新增 grammar 版本带来新 kind 时会立刻失败，而不是默默漏事件。
#[test]
fn every_kind_is_classified() {
    for name in ["typescript", "python"] {
        let src = SAMPLES.iter().find(|(n, _)| *n == name).unwrap().1;
        let lang = if name == "python" { Language::Python } else { Language::TypeScript };
        let unclassified: Vec<String> = dump(lang, src).into_iter()
            .filter(|k| !ast::is_classified(k))
            .collect();
        assert!(unclassified.is_empty(), "{} 未被任何词表分类: {unclassified:?}", name);
    }
}
```

- [ ] **Step 2: 跑探测，人工核对生成的 kinds.tsv**

```bash
cargo test --locked nodekinds 2>&1 | tail -40
```

Expected: 第一次 panic 并生成 `tests/fixtures/typescript.kinds.tsv` 与 `python.kinds.tsv`。**打开这两个文件逐个核对**：每个 kind 归入下面五张表之一（`control_flow` / `guard` / `error_handling` / `import` / `test_declaration`），或明确判为"不参与事件"。把核对结论写进 `src/ast/scopes.rs` 的注释里。**这一步不能跳，也不能信任计划里写的 kind 名。**

- [ ] **Step 3: 词表**

`src/ast/mod.rs`：

```rust
//! tree-sitter 接线。spec §5 的 node_kind / scope 来源。

pub mod pair;
pub mod scopes;

pub use scopes::*;

use crate::evidence::Language;
use crate::seir::ScopeRef;
use tree_sitter::{InputEdit, Node, Parser, Tree};

#[derive(Debug, thiserror::Error)]
pub enum AstError {
    #[error("tree-sitter rejected the language")]
    LanguageRejected,
    #[error("parser error node present: {0} bytes of ERROR")]
    HasErrorNodes(usize),
}

pub struct ParsedFile {
    pub language: Language,
    source: String,
    tree: Tree,
}

impl ParsedFile {
    /// **完整文件**必须用整串解析。hunk 片段不得走这个函数——
    /// 片段解析会产生 ERROR 节点，spec §6 要求它降到 no_ast_match 而不是当成结构。
    pub fn parse(language: Language, source: &str) -> Result<Self, AstError> {
        let mut parser = Parser::new();
        let ts_lang = match language {
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
            Language::JavaScript => tree_sitter_javascript::LANGUAGE,
            Language::Python => tree_sitter_python::LANGUAGE,
        };
        parser.set_language(&ts_lang).map_err(|_| AstError::LanguageRejected)?;
        let tree = parser.parse(source, None).ok_or(AstError::LanguageRejected)?;
        let mut bytes = 0usize;
        let mut stack = vec![tree.root_node()];
        while let Some(n) = stack.pop() {
            if n.is_error() {
                bytes += (n.end_byte() - n.start_byte()) as usize;
            }
            for c in n.children(&mut n.walk()) {
                stack.push(c);
            }
        }
        if bytes > 0 {
            return Err(AstError::HasErrorNodes(bytes));
        }
        Ok(Self { language, source: source.to_string(), tree })
    }

    pub fn root(&self) -> Node<'_> {
        self.tree.root_node()
    }

    pub fn text_of<'a>(&'a self, node: Node<'a>) -> &'a str {
        &self.source[node.start_byte()..node.end_byte()]
    }

    /// 1-based 行号 + 0-based 列。
    pub fn line_col_of(&self, byte: usize) -> (u32, u32) {
        let n = self.root().descendant_for_byte_range(byte, byte + 1);
        match n {
            Some(node) => {
                let p = node.start_position();
                (p.row as u32 + 1, p.column as u32)
            }
            None => {
                let mut acc = 0u32;
                let bytes = self.source.as_bytes();
                for b in bytes.iter().take(byte) {
                    if *b == b'\n' { acc += 1; }
                }
                (acc + 1, 0)
            }
        }
    }

    /// 供 Task 12：片段解析失败时降级。
    pub fn has_error_nodes(&self) -> bool {
        let mut stack = vec![self.root()];
        while let Some(n) = stack.pop() {
            if n.is_error() { return true; }
            for c in n.children(&mut n.walk()) { stack.push(c); }
        }
        false
    }
}

/// Task 12：hunk 片段解析。片段必然不完整，故**不拒绝 ERROR 节点**，
/// 但调用方必须据此把事件标 `no_ast_match` 并降到 hunk_only。
pub fn parse_fragment(language: Language, fragment: &str) -> Option<ParsedFile> {
    ParsedFile::parse_lenient(language, fragment)
}
```

`ParsedFile::parse_lenient` 是 `parse` 的副本但**去掉 `HasErrorNodes` 早退**（仍返回 `Result`，只在 `LanguageRejected` 时失败）；`InputEdit` 若未使用就从 `tree_sitter::{...}` 的 import 里删掉，否则 clippy `-D warnings` 挂。语法 crate 的 `LANGUAGE` 常量名（`tree_sitter_typescript::LANGUAGE_TYPESCRIPT` / `tree_sitter_javascript::LANGUAGE`）在各版本间不一致，**Task 6 Step 1 加依赖后立刻用 `cargo doc -p tree-sitter-typescript` 或编译器报错确认实际名字**，不要照抄计划。

`src/ast/scopes.rs`：**内容以 Step 2 生成的 kinds.tsv 为准**。下面是结构，不是最终名单：

```rust
//! node kind 词表。名单来源：tests/fixtures/{typescript,python}.kinds.tsv（Task 6 Step 1 实测生成）。
//! 新增 grammar 版本带来新 kind 时 every_kind_is_classified 会失败——那是提醒，不是障碍。

use crate::seir::ScopeRef;
use tree_sitter::Node;

/// 定义类节点：`scope` 候选。必须是能带名字的。
pub const DEFINITION_KINDS: &[&str] = &[
    "function_declaration", "generator_function_declaration", "class_declaration",
    "abstract_class_declaration", "method_definition", "interface_declaration",
    "enum_declaration", "type_alias_declaration", "module",
    "function_definition", "class_definition", "decorated_definition",
];

pub const CONTROL_FLOW_KINDS: &[&str] = &[
    "if_statement", "for_statement", "for_in_statement", "while_statement",
    "do_statement", "switch_statement", "match_statement", "try_statement",
    "with_statement",
];

/// guard = 早退型条件。仅靠 kind 不够，必须看 consequent（见 kind_is_guard）。
pub const GUARD_CANDIDATE_KINDS: &[&str] = &["if_statement", "guard_condition", "assert_statement"];

pub const ERROR_HANDLING_KINDS: &[&str] = &[
    "try_statement", "catch_clause", "catch_error_clause", "finally_clause",
    "throw_statement", "except_clause", "raise_statement", "assert_statement",
];

pub const IMPORT_KINDS: &[&str] = &[
    "import_statement", "import_from_statement", "call",  // 仅 `import()` / `__import__`
];

pub const TEST_DECLARATION_KINDS: &[&str] = &["function_declaration", "function_definition"];

/// 显式承认"不参与事件"的 kind，供 every_kind_is_classified 通过。
pub const NON_EVENT_KINDS: &[&str] = &[
    "program", "expression_statement", "statement_block", "comment", "export_statement",
    "lexical_declaration", "variable_declaration", "assignment", "pair", "property_signature",
    "public_field_definition", "field_definition", "formal_parameters", "required_parameter",
    "optional_parameter", "parameters", "type_identifier", "return_statement", "break_statement",
    "continue_statement", "arrow_function", "function", "generator_function", "class",
    "identifier", "type_annotation", "string", "number", "true", "false", "null", "undefined",
    "ERROR", "MISSING",
];

pub fn is_classified(kind: &str) -> bool {
    DEFINITION_KINDS.contains(&kind)
        || CONTROL_FLOW_KINDS.contains(&kind)
        || GUARD_CANDIDATE_KINDS.contains(&kind)
        || ERROR_HANDLING_KINDS.contains(&kind)
        || IMPORT_KINDS.contains(&kind)
        || TEST_DECLARATION_KINDS.contains(&kind)
        || NON_EVENT_KINDS.contains(&kind)
}

pub fn kind_is_control_flow(kind: &str) -> bool {
    CONTROL_FLOW_KINDS.contains(&kind)
}

pub fn kind_is_error_handling(kind: &str) -> bool {
    ERROR_HANDLING_KINDS.contains(&kind)
}

/// guard 的判定不能只看 kind：`if (a) doThing()` 是 control_flow，
/// `if (!a) return;` 才是 guard。区别在 consequent 是否只含一条早退语句。
pub fn kind_is_guard(node: Node<'_>) -> bool {
    match node.kind() {
        "assert_statement" => true,
        "if_statement" => node
            .child_by_field_name("consequence")
            .map(is_single_early_exit_block)
            .unwrap_or_else(|| {
                // 兜底：该 grammar 若没暴露 `consequence` field，取最后一个 named child。
                // tests/ast.rs::guard_is_distinguished_from_control_flow 会钉住哪条分支有效。
                let n = node.named_child_count();
                if n == 0 { return false; }
                node.named_child(n - 1).map(is_single_early_exit_block).unwrap_or(false)
            }),
        _ => false,
    }
}

fn is_single_early_exit_block(cons: Node<'_>) -> bool {
    if !matches!(cons.kind(), "statement_block" | "compound_statement") && cons.named_child_count() > 1 {
        // 裸语句（无花括号）的 consequent 本身就是那条语句
        return is_early_exit(&cons);
    }
    let mut c = cons.walk();
    let kids: Vec<Node<'_>> = cons.named_children(&mut c).collect();
    kids.len() == 1 && is_early_exit(&kids[0])
}

fn is_early_exit(n: &Node<'_>) -> bool {
    matches!(n.kind(), "return_statement" | "throw_statement" | "raise_statement")
}
```

若 Step 2 的 kinds.tsv 显示 `if_statement` 的 consequent field 名不同，**改这个函数，不改测试期望**——`guard_is_distinguished_from_control_flow` 的两条断言（早退 if 是 guard、带副作用 if 不是）是判据本身。

/// 从节点取定义名。不同 grammar 的名字位置不同，按 kind 分支。
pub fn definition_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut c = node.walk();
    node.named_children(&mut c).find(|ch| ch.kind() == "identifier" || ch.kind() == "type_identifier" || ch.kind() == "property_identifier")
        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
}

pub fn scope_ref(node: Node<'_>, source: &str) -> Option<ScopeRef> {
    let name = definition_name(node, source)?;
    Some(ScopeRef {
        name,
        node_kind: node.kind().to_string(),
        range: (
            node.start_position().row as u32 + 1,
            node.end_position().row as u32 + 2,
        ),
    })
}

/// 定义节点，深度优先，按 (行, 列, 名) 规范序。spec §4.1。
pub fn definitions(pf: &ParsedFile) -> Vec<(ScopeRef, Node<'_>)> {
    todo!("DFS，DEFINITION_KINDS 命中即收集；嵌套定义都收集（内层 scope 用外层的名字）")
}

/// 找 byte 处的最内层 enclosing definition。
pub fn enclosing_scope(pf: &ParsedFile, byte: usize) -> Option<ScopeRef> {
    todo!("从 node_at(byte) 往上走，第一个 DEFINITION_KINDS 命中者")
}
```

`definitions` / `enclosing_scope` 在 Step 3 里是 `todo!`——**Step 4 的 8 条测试必须先跑起来（编译过、断言失败），再用实现让它们变绿**，提交前不得残留 `todo!`（`clippy` 的 `-D warnings` 与 `cargo test` 都会拦）。两者的算法已在 Step 3 的注释里给全：`definitions` 走 DFS 收 `DEFINITION_KINDS` 命中者（嵌套定义也收，内层用外层名做 `scope` 链）；`enclosing_scope` 从 `descendant_for_byte_range` 起沿 parent 上溯到第一个 `DEFINITION_KINDS` 命中者。

- [ ] **Step 4: scope 解析测试**

`tests/ast.rs`：

```rust
use diffintent::{ast, evidence::Language};

fn ts(src: &str) -> ast::ParsedFile {
    ast::ParsedFile::parse(Language::TypeScript, src).unwrap()
}
fn py(src: &str) -> ast::ParsedFile {
    ast::ParsedFile::parse(Language::Python, src).unwrap()
}

#[test]
fn enclosing_scope_finds_the_function() {
    let pf = ts("export function authenticate(t: string) {\n  const x = 1;\n  return x;\n}\n");
    let scope = ast::enclosing_scope(&pf, pf.source.find("const x").unwrap()).unwrap();
    assert_eq!(scope.name, "authenticate");
    assert_eq!(scope.node_kind, "function_declaration");
    assert_eq!(scope.range, (1, 5));
}

#[test]
fn method_and_class_nesting() {
    let pf = ts("class A {\n  m() {\n    return 1;\n  }\n}\n");
    let s = ast::enclosing_scope(&pf, pf.source.find("return").unwrap()).unwrap();
    assert_eq!(s.name, "m");
    assert_eq!(s.node_kind, "method_definition");
}

/// 模块级语句没有 enclosing definition → None（Task 8 归入 scope: null 的 block）。
#[test]
fn module_level_has_no_scope() {
    let pf = ts("import { x } from \"y\";\nexport const a = 1;\n");
    assert!(ast::enclosing_scope(&pf, 0).is_none());
}

#[test]
fn python_function_and_async() {
    let pf = py("def f():\n    return 1\n\nasync def g():\n    await f()\n");
    assert_eq!(ast::enclosing_scope(&pf, pf.source.find("return").unwrap()).unwrap().name, "f");
    assert_eq!(ast::enclosing_scope(&pf, pf.source.find("await").unwrap()).unwrap().name, "g");
}

/// 嵌套函数：最内层优先。
#[test]
fn innermost_wins() {
    let pf = py("def outer():\n    def inner():\n        return 1\n    return inner\n");
    assert_eq!(ast::enclosing_scope(&pf, pf.source.find("return 1").unwrap()).unwrap().name, "inner");
}

#[test]
fn definitions_are_canonically_ordered() {
    let pf = ts("function b() {}\nfunction a() {}\nclass C { m() {} }\n");
    let defs = ast::definitions(&pf);
    let names: Vec<&str> = defs.iter().map(|(s, _)| s.name.as_str()).collect();
    assert_eq!(names, vec!["b", "a", "C", "m"], "DFS 顺序，非字典序（spec §4.1 的排序键是行/列）");
}

/// 空文件与只有注释都必须可解析。
#[test]
fn degenerate_inputs_parse() {
    assert!(ast::ParsedFile::parse(Language::TypeScript, "").is_ok());
    assert!(ast::ParsedFile::parse(Language::TypeScript, "// only a comment\n").is_ok());
    assert!(ast::ParsedFile::parse(Language::Python, "").is_ok());
}

/// 真正的语法错误必须早退（full_ast 路径不允许带 ERROR 节点建事件）。
#[test]
fn syntax_error_is_rejected() {
    let e = ast::ParsedFile::parse(Language::TypeScript, "function ( {{{").unwrap_err();
    assert!(matches!(e, ast::AstError::HasErrorNodes(_) | ast::AstError::LanguageRejected), "{e:?}");
}

/// 片段解析故意宽松（Task 12 依赖这一点），但必须报告有 ERROR。
#[test]
fn fragment_parsing_is_lenient_but_reports() {
    let frag = "  if (!v) {\n    return 0;\n";  // 不完整
    let pf = ast::parse_fragment(Language::TypeScript, frag).expect("lenient parse must not fail");
    assert!(pf.has_error_nodes());
}

#[test]
fn guard_is_distinguished_from_control_flow() {
    let pf = ts("function f(a) {\n  if (!a) return 0;\n  if (a > 1) { doThing(); }\n}\n");
    let guard = pf.source.find("if (!a)").unwrap();
    let other = pf.source.find("if (a > 1)").unwrap();
    let n1 = pf.root().descendant_for_byte_range(guard, guard + 2).unwrap();
    let n2 = pf.root().descendant_for_byte_range(other, other + 2).unwrap();
    assert!(ast::kind_is_guard(n1, &pf.source), "早退 if 是 guard");
    assert!(!ast::kind_is_guard(n2, &pf.source), "带副作用的 if 只是 control_flow");
}
```

`ParsedFile.source` 需 `pub`（`src/ast/mod.rs` 里把 `source: String` 改为 `pub source: String`）。`scope.range` 的终点用 `end_position().row + 2`（闭开区间的"下一行行号"语义）——这个约定必须在 `tests/ast.rs::enclosing_scope_finds_the_function` 里钉住，并在 `src/seir/mod.rs` 的 `ScopeRef` 文档注释里写明。

Run: `cargo test --locked` → 全 PASS（含 nodekinds 两个）。

- [ ] **Step 5: definition 配对（Task 9/10/11 的共同骨架）**

- [ ] **Step 5: 先建立模块与类型（不含实现）**

`src/ast/pair.rs`：

```rust
//! 新旧 definition 配对。配对错会同时污染 signature/binding/call/control_flow 四类事件。
//! 规则见 Step 6：先按 (name, node_kind) 精确配对，剩余按同 node_kind 的出现顺序位置配对，
//! 再剩余的才是真 Added/Removed。**不做相似度匹配**（moved 检测明确排除，spec §5.2）。

use crate::seir::ScopeRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pair {
    Matched { old: usize, new: usize },
    Added { new: usize },
    Removed { old: usize },
}

/// 实现见 Step 6 —— 本步骤只建类型，让 `src/ast/mod.rs` 的 `pub mod pair;` 先编译通过。
pub fn pair_definitions(_old: &[ScopeRef], _new: &[ScopeRef]) -> Vec<Pair> {
    Vec::new()
}
```

**Step 5 不单独提交**：它与 Step 6 是同一次提交（`pub mod pair;` 一旦声明就必须存在，而空实现会让 Step 6 的测试立刻失败——那是 TDD 想要的顺序，但不是可提交的状态）。

- [ ] **Step 6: pair 实现与测试**

`src/ast/pair.rs` 替换 `pair_definitions`：

```rust
pub fn pair_definitions(old: &[ScopeRef], new: &[ScopeRef]) -> Vec<Pair> {
    let mut used_old = vec![false; old.len()];
    let mut pairs: Vec<Pair> = Vec::new();

    // pass 1: 同名同类型精确配对
    for (ni, n) in new.iter().enumerate() {
        for (oi, o) in old.iter().enumerate() {
            if !used_old[oi] && o.name == n.name && o.node_kind == n.node_kind {
                used_old[oi] = true;
                pairs.push(Pair::Matched { old: oi, new: ni });
                break;
            }
        }
    }
    // pass 2: 剩余的按 (node_kind, 出现顺序) 位置配对，吸收"整体下移/上移"噪声
    let free_new: Vec<usize> = (0..new.len()).filter(|ni| !pairs.iter().any(|p| matches!(p, Pair::Matched{new, ..} if *new == *ni))).collect();
    let free_old: Vec<usize> = (0..old.len()).zip(used_old.iter()).filter(|(_, u)| !**u).map(|(i, _)| i).collect();
    for (k, ni) in free_new.iter().enumerate() {
        if let Some(oi) = free_old.get(k).copied().filter(|oi| old[*oi].node_kind == new[*ni].node_kind) {
            pairs.push(Pair::Matched { old: oi, new: *ni });
        } else {
            pairs.push(Pair::Added { new: *ni });
        }
    }
    let matched_new: Vec<usize> = pairs.iter().filter_map(|p| match p { Pair::Matched{new, ..} => Some(*new), _ => None }).collect();
    let matched_old: Vec<usize> = pairs.iter().filter_map(|p| match p { Pair::Matched{old, ..} => Some(*old), _ => None }).collect();
    for oi in 0..old.len() {
        if !matched_old.contains(&oi) { pairs.push(Pair::Removed { old: oi }); }
    }
    for ni in 0..new.len() {
        if !matched_new.contains(&ni) && !pairs.iter().any(|p| matches!(p, Pair::Added{new} if *new == ni)) {
            pairs.push(Pair::Added { new: ni });
        }
    }
    pairs.sort_by_key(|p| match p {
        Pair::Matched { new, .. } | Pair::Added { new } => (*new, 0u8),
        Pair::Removed { old } => (*old, 1u8),
    });
    pairs
}
```

`tests/pair.rs`：

```rust
use diffintent::ast::pair::{pair_definitions, Pair};
use diffintent::seir::ScopeRef;

fn s(name: &str, kind: &str, line: u32) -> ScopeRef {
    ScopeRef { name: name.into(), node_kind: kind.into(), range: (line, line + 3) }
}

#[test]
fn exact_name_and_kind_match() {
    let p = pair_definitions(&[s("a", "function_declaration", 1)], &[s("a", "function_declaration", 5)]);
    assert_eq!(p, vec![Pair::Matched { old: 0, new: 0 }]);
}

#[test]
fn added_and_removed_are_separate() {
    let p = pair_definitions(&[s("a", "function_declaration", 1)], &[s("b", "function_declaration", 1)]);
    // 同为 function_declaration 但名字不同：pass 2 的位置配对会误判成 Matched
    // 这条测试存在的目的就是这个歧义 —— 落地时必须决定：pass 2 是否只用于匿名/同名节点？
    assert!(p.iter().any(|x| matches!(x, Pair::Added { .. })), "{p:?}");
    assert!(p.iter().any(|x| matches!(x, Pair::Removed { .. })), "{p:?}");
}

#[test]
fn different_kind_never_matches() {
    let p = pair_definitions(&[s("a", "class_declaration", 1)], &[s("a", "interface_declaration", 1)]);
    assert_eq!(p, vec![Pair::Added { new: 0 }, Pair::Removed { old: 0 }]);
}

/// 纯位置下移不得报成删+加。
#[test]
fn pure_reordering_is_not_add_remove() {
    let old = vec![s("a", "function_declaration", 1), s("b", "function_declaration", 10)];
    let new = vec![s("a", "function_declaration", 1), s("b", "function_declaration", 20)];
    let p = pair_definitions(&old, &new);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(p.iter().all(|x| matches!(x, Pair::Matched { .. })));
}

#[test]
fn empty_sides() {
    assert_eq!(pair_definitions(&[], &[s("a", "function_declaration", 1)]), vec![Pair::Added { new: 0 }]);
    assert_eq!(pair_definitions(&[s("a", "function_declaration", 1)], &[]), vec![Pair::Removed { old: 0 }]);
    assert_eq!(pair_definitions(&[], &[]), vec![]);
}
```

**`added_and_removed_are_separate` 会揭穿 pass 2 的位置配对**：两个不同名的同名类型函数会被配成 `Matched`。决策（写进 spec §5.1 的注释并在此实现）：**pass 2 只在 `node_kind` 相同且两侧剩余数量相等且 `name` 都是空串或同一匿名节点时启用**——否则名字不同就是删+加。按此收紧实现，让测试过。

Run: `cargo test --locked` → 全 PASS。

- [ ] **Step 7: 体积冒烟检查**

```bash
cargo build --release && bash scripts/check-budget.sh target/release/diffintent
```

Expected: 四个语法包静态链接后仍 < 12 MB，冷启动 < 160 ms。**这是 spec §6 "tree-sitter 体积陷阱"的第一次实测**。若超预算：先试 `panic = "abort"` + `strip` + `opt-level = "z"` 的 `codegen-units = 1` 组合，再考虑 TS/TSX 只留一个语法包；**不要在 W1 内改成运行时下载语法**（那是 spec §6 明令禁止的"假装支持"反面——它会让离线 CI 静默降级）。把实测数字写进 commit body。

- [ ] **Step 8: 提交**

```bash
git add src/ast tests/ast.rs tests/pair.rs tests/nodekinds.rs tests/fixtures Cargo.toml Cargo.lock build.rs
git commit -m "feat: tree-sitter 接线、node kind 探测表与 definition 配对

kind 名单由 tests/fixtures/*.kinds.tsv 实测生成而非凭记忆手写，且 every_kind_is_classified
让 grammar 升级引入新 kind 时立刻失败，而不是默默漏事件。
完整文件拒绝 ERROR 节点；片段解析故意宽松但必须报告 has_error_nodes。
实测体积/冷启动：<填入数字>。"
```

---

### Task 7: 排除层（token 流折叠与 rename 折叠）

覆盖 spec §5.2。原书 §3 说"借用 intentdiff 的思路作为排除层"——这里只借判据，不借它的分类体系。两个判据都便宜且精确：`formatting` = 新旧 **token 流逐位相等**；`rename` = 直接读 git 的 `rename from/to` 元数据行。

**Files:**
- Create: `src/events/lex.rs`
- Modify: `src/events/mod.rs`（新建，本任务先只放 `fold` 相关）
- Test: `tests/lex_fold.rs`

**Interfaces:**
- Consumes: Task 6 的 `ParsedFile`
- Produces:
  - `events::lex::token_stream(&ParsedFile) -> Vec<String>`（跳过空白与注释）
  - `events::lex::is_formatting_only(old: &ParsedFile, new: &ParsedFile) -> bool`
  - `events::lex::comment_texts(&ParsedFile) -> Vec<String>`
  - `events::fold::rename_only(file: &FileDiff) -> bool`
  - `events::fold::Folded { formatting: usize, renames: usize, comments_only: usize }`

- [ ] **Step 1: 写失败测试**

`tests/lex_fold.rs`：

```rust
use diffintent::{ast::ParsedFile, evidence::Language, events::lex::*, patch};

fn ts(src: &str) -> ParsedFile { ParsedFile::parse(Language::TypeScript, src).unwrap() }

/// 格式化扰动不得产生任何意图信号（spec §3.1：折叠）。
#[test]
fn whitespace_and_semicolon_changes_are_formatting_only() {
    let a = ts("function f(a) {\n  return a;\n}\n");
    let b = ts("function f ( a ) {\n\n    return a\n\n}\n");
    assert!(is_formatting_only(&a, &b), "token 流 {token_stream(&a):?} vs {token_stream(&b):?}");
}

/// 缩进宽度与尾随空白同理。
#[test]
fn indentation_only_is_formatting() {
    let a = ts("function f() {\n\treturn 1;\n}\n");
    let b = ts("function f() {\n    return 1;\n}\n");
    assert!(is_formatting_only(&a, &b));
}

/// 加引号风格变化（单双引号在 JS 里是不同 token）——**不是** formatting。
/// 这条测试存在的目的：阻止把"看起来一样"当判据。
#[test]
fn quote_style_change_is_not_formatting() {
    let a = ts("const s = 'x';\n");
    let b = ts("const s = \"x\";\n");
    assert!(!is_formatting_only(&a, &b));
}

/// 改标识符不是格式化。
#[test]
fn rename_identifier_is_not_formatting() {
    let a = ts("function f(value) { return value; }\n");
    let b = ts("function f(v) { return v; }\n");
    assert!(!is_formatting_only(&a, &b));
}

/// 只加/删注释 → comment 折叠，不算格式化也不算结构变更。
#[test]
fn comment_only_change_is_not_formatting_but_has_comment_diff() {
    let a = ts("function f() { return 1; }\n");
    let b = ts("// explain\nfunction f() { return 1; }\n");
    assert!(!is_formatting_only(&a, &b));
    assert_eq!(comment_texts(&a).len(), 0);
    assert_eq!(comment_texts(&b).len(), 1);
}

/// Python 的注释 kind 与 JS 不同，两张表都得对。
#[test]
fn python_comments_are_collected() {
    let a = ParsedFile::parse(Language::Python, "def f():\n    # note\n    return 1\n").unwrap();
    assert_eq!(comment_texts(&a), vec!["# note"]);
}

#[test]
fn token_stream_ignores_comments_entirely() {
    let a = ts("function f() {\n  // hidden\n  return 1;\n}\n");
    let b = ts("function f() {\n  return 1;\n}\n");
    assert_eq!(token_stream(&a), token_stream(&b), "注释不得进 token 流");
}

/// rename-only diff（有 rename from/to 且无内容 hunk）必须被折叠。
#[test]
fn git_rename_metadata_folds() {
    let text = std::fs::read_to_string("tests/golden/rename.diff").unwrap();
    let files = patch::parse(&text).unwrap();
    let pure = files.iter().any(|f| patch::FileChange::is_rename(&f.change));
    assert!(pure, "夹具必须含 rename 元数据行");
    assert!(diffintent::events::fold::rename_only(&files[0]) || !files[0].hunks.is_empty());
}

/// 无 git 扩展头的 diff（patch -p1）认不出 rename，**不得假装折叠**。
#[test]
fn no_extension_header_cannot_fold_rename() {
    let text = "--- a/src/x.ts\n+++ b/src/x.ts\n@@ -1 +1 @@\n-a\n+b\n";
    let f = patch::parse(text).unwrap().remove(0);
    assert!(!f.has_git_extension_header);
    assert!(!diffintent::events::fold::rename_only(&f));
}
```

Run: `cargo test --locked --test lex_fold` → Expected: 编译失败。

- [ ] **Step 2: token 流**

`src/events/lex.rs`：

```rust
//! spec §5.2 排除层之一：token 流逐位相等即格式化。
//! 用 tree-sitter 的 token 边界而非正则——后者无法区分 `'x'` 与 `"x"`、
//! 也无法区分注释和字符串里的空格。

use super::super::ast::ParsedFile;
use tree_sitter::TreeCursor;

pub fn token_stream(pf: &ParsedFile) -> Vec<String> {
    let mut out = Vec::new();
    let root = pf.root();
    let mut c = root.walk();
    walk_tokens(pf, root, &mut c, &mut out);
    out
}

fn walk_tokens<'a>(pf: &'a ParsedFile, node: tree_sitter::Node<'a>, c: &mut TreeCursor<'a>, out: &mut Vec<String>) {
    if node.kind() == "comment" {
        return;  // 注释不进 token 流
    }
    if node.child_count() == 0 {
        out.push(pf.text_of(node).to_string());
        return;
    }
    if c.goto_first_child() {
        loop {
            walk_tokens(pf, c.node(), c, out);
            if !c.goto_next_sibling() { break; }
        }
        c.goto_parent();
    }
}

pub fn is_formatting_only(old: &ParsedFile, new: &ParsedFile) -> bool {
    old.language == new.language && token_stream(old) == token_stream(new)
}

pub fn comment_texts(pf: &ParsedFile) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![pf.root()];
    while let Some(n) = stack.pop() {
        if n.kind() == "comment" {
            out.push(pf.text_of(n).trim().to_string());
        }
        let mut c = n.walk();
        for ch in n.children(&mut c) { stack.push(ch); }
    }
    out.sort();  // 注释顺序不得影响结果
    out
}
```

`walk_tokens` 借用检查大概率会挂（`TreeCursor` 与 `node` 同源双借用）。**落地方案：不用 cursor，改用 `node.named_children(&mut node.walk())` 递归 + `node.child_count()`** 判断是否为 anonymous token，或者更简单——**对每个 leaf（`node.child_count() == 0`）收集文本**。写法：

```rust
fn collect_leaves<'a>(pf: &'a ParsedFile, node: tree_sitter::Node<'a>, out: &mut Vec<String>) {
    if node.kind() == "comment" { return; }
    if node.child_count() == 0 {
        out.push(pf.text_of(node).to_string());
        return;
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        collect_leaves(pf, ch, out);
    }
}
```

注意 `children(&mut c)` 借用 `c`，递归体内不得再用同一个 cursor——上面的写法每次递归都 `node.walk()` 新建 cursor，代价可接受（W1 只走 full_ast 的两棵完整树）。若性能预算（Task 14）不过，改成显式栈。

- [ ] **Step 3: rename 折叠**

`src/events/mod.rs`：

```rust
//! spec §4 第 3 段。检测器是统一签名的函数，按固定顺序存在 static 数组里。

pub mod fold;
pub mod lex;

pub use lex::{comment_texts, is_formatting_only, token_stream};

pub fn extract_events(
    files: &[diffintent::evidence::ResolvedFile],
) -> Vec<crate::seir::Event> {
    // Task 8 填实。此处返回空，让 Task 7 独立可交付。
    let _ = files;
    Vec::new()
}
```

`src/events/fold.rs`：

```rust
use crate::patch::{FileChange, FileDiff};

/// 纯改名：有 git rename 元数据 **且** 无内容 hunk。
/// 折叠 = 不产生任何结构事件、不参与意图判断、不参与 drift。
pub fn rename_only(file: &FileDiff) -> bool {
    file.has_git_extension_header
        && matches!(file.change, FileChange::Renamed { .. })
        && file.hunks.is_empty()
}

/// rename + 微改：改名本身折叠，内容部分照走正常检测器。
/// 这是 intentdiff 那套分类与本产品边界最容易混的地方，写成测试钉住。
pub fn split_rename_with_edit(file: &FileDiff) -> (bool, bool) {
    let renamed = matches!(file.change, FileChange::Renamed { .. }) && file.has_git_extension_header;
    (renamed, !file.hunks.is_empty())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Folded {
    pub formatting: usize,
    pub renames: usize,
    pub comments_only: usize,
}
```

`src/patch/model.rs` 补 `impl FileChange { pub fn is_rename(&self) -> bool { matches!(self, Self::Renamed { .. }) } }`。

- [ ] **Step 4: 跑测试**

`cargo test --locked` → 全 PASS。`quote_style_change_is_not_formatting` 是关键：若它挂了，说明 token 流把 `'x'` 和 `"x"` 归一化了——**归一化是错的**，那会让 `"const s = 'x'"` → `"const s = \"x\""` 这种真实的行为改动被静默吞掉。

- [ ] **Step 5: 提交**

```bash
git add src/events tests/lex_fold.rs src/patch/model.rs
git commit -m "feat: 排除层（token 流格式化折叠 + git rename 折叠）

用 tree-sitter 的 token 边界而非正则：正则区分不了 'x' 与 \"x\"，
也区分不了注释和字符串里的空格。quote 风格变化必须是**非**格式化。
无 git 扩展头的 diff 认不出 rename，不得假装折叠。"
```

---

### Task 8: 检测器框架与文件级事件

覆盖 spec §4.2（`static DETECTORS`，不用 trait object）+ §5.1 的 `file` `dependency` `module` `config_key` 四类。这四类先做是因为它们**全部是 drift 高精确触发器的输入**（spec §9.3），且都不依赖 definition 配对。

**Files:**
- Create: `src/events/detect.rs`, `src/events/file_level.rs`
- Create: `tests/golden/01_new_metrics.ts.diff` + `.events.json`, `tests/golden/02_dep_added.ts.diff` + `.events.json`, `tests/golden/03_file_deleted.ts.diff` + `.events.json`, `tests/golden/04_config_touched.json.diff` + `.events.json`, `tests/golden/05_local_alias_import_is_not_dependency.diff` + `.events.json`
- Test: `tests/golden_events.rs`, `tests/detectors_file.rs`

**Interfaces:**
- Consumes: Task 5 `ResolvedFile`；Task 6 `ParsedFile` `IMPORT_KINDS`；Task 7 `fold`；Task 2 `Event`/`EventSink`
- Produces:
  - `events::detect::DetectorCtx<'a> { file: &'a ResolvedFile, old: Option<&'a ParsedFile>, new: Option<&'a ParsedFile> }`
  - `events::detect::EventSink { fn push(&mut self, Event) }`（内部 `Vec<Event>`，**不是** `HashSet`）
  - `events::detect::DETECTORS: &[DetectorFn]`（顺序即执行顺序，写死并测试）
  - `events::extract_events(&[ResolvedFile]) -> Vec<Event>`
  - `events::detect::is_third_party_module(&str, &FileDiff) -> bool`
  - `events::detect::is_test_path(&str) -> bool`
  - `tests/golden_events.rs::run_case(name) -> Vec<Event>`

- [ ] **Step 1: 定金标准格式与跑法**

`tests/golden/<n>.events.json` 的格式就是**规范序事件的 serde 输出**，不加任何专有字段——金标准必须与 `--dump-events` 完全同源，否则 golden 通过而实际输出不一致：

```json
[
  {
    "file": 0,
    "kind": "dependency",
    "direction": "added",
    "scope": null,
    "node_kind": "import_statement",
    "range": [4, 5],
    "tier": "full_ast"
  }
]
```

`tests/golden_events.rs`：

```rust
//! 金标准语料跑法。三个关键设计：
//! 1) 期望值就是 serde_json 序列化结果，与 --dump-events 同源，不存在第二套渲染。
//! 2) 每个 case 需要一个配套工作树（full_ast 的前提），由 xtask gendev 生成、进仓库。
//! 3) 期望事件为空时也必须写 `[]`，不能缺文件——缺文件会让"该 case 什么都没产"
//!    与"该 case 忘了加"两种状态无法区分。

use diffintent::{evidence::*, events, patch, seir::*};
use std::{fs, path::Path};

pub fn run_case(case: &str) -> Vec<Event> {
    let diff = fs::read_to_string(format!("tests/golden/{case}.diff")).expect("diff fixture");
    let wt_root = Path::new("tests/golden_worktree").join(case);
    assert!(wt_root.exists(), "缺工作树 {wt_root:?}：跑 cargo run -p xtask -- gendev 并 commit");
    let files = patch::parse(&diff).expect("parse");
    let wt = Worktree::new(Some(wt_root)).expect("worktree");
    let resolved = resolve(files, Some(&wt));
    // 前提断言：金标准全部要求 full_ast。若某文件降到 hunk_only，
    // 说明 gendev 产物与 .diff 漂移了——报错并提示重生成，不要改期望值。
    for r in &resolved {
        assert_eq!(r.tier, EvidenceTier::FullAst,
            "case {case} 文件 {} 不是 full_ast（{:?}）：重跑 xtask gendev", r.diff.newest_path(), r.reason);
    }
    let mut got = events::extract_events(&resolved);
    canonical_sort(&mut got);
    got
}

fn expect_of(case: &str) -> Vec<Event> {
    let raw = fs::read_to_string(format!("tests/golden/{case}.events.json")).expect("events fixture");
    serde_json::from_str(&raw).expect("valid events json")
}

fn check(case: &str) {
    let got = run_case(case);
    let want = expect_of(case);
    let gs = serde_json::to_string_pretty(&got).unwrap();
    let ws = serde_json::to_string_pretty(&want).unwrap();
    assert_eq!(gs, ws, "case {case}");
}

#[test] fn new_metrics_module_is_module_added() { check("01_new_metrics"); }
#[test] fn added_dep_is_dependency_added() { check("02_dep_added"); }
#[test] fn deleted_file_is_file_removed() { check("03_file_deleted"); }
#[test] fn config_edit_is_config_key() { check("04_config_touched"); }
#[test] fn local_alias_is_not_a_dependency() { check("05_local_alias_import"); }
```

`Event` 的 serde 表示需 `range` 序列化成数组（`Vec<u32>` 形态）——在 `src/seir/mod.rs` 给 `Event` 加 `#[serde(rename_all = "snake_case")]` 并确认 `(u32, u32)` 默认就序列化成 `[a, b]`（Rust 元组的 serde 实现确实如此）。`EvidenceTier` 与 `EventKind`/`Direction` 已带 `rename_all = "snake_case"`。

- [ ] **Step 2: 写失败测试（框架部分）**

`tests/detectors_file.rs`：

```rust
use diffintent::events::detect::*;

/// DETECTORS 顺序是规范序的一部分（spec §4.2）。重排必须失败。
#[test]
fn detector_order_is_frozen() {
    let names: Vec<&str> = DETECTORS.iter().map(|d| d.name).collect();
    assert_eq!(names, vec![
        "file_level",
        "dependency",
        "config_key",
        // Task 9/10/11 依次追加，追加时同步改这条断言
    ]);
}

/// 相对与别名 import 不算第三方依赖（spec §9.3 HP1 的 (a) 条）。
#[test]
fn local_and_alias_imports_are_not_third_party() {
    for m in ["./x", "../y", "@/foo", "~/bar", "#/internal", "src/util", "./"] {
        assert!(!is_third_party_module(m, "src/a.ts"), "{m}");
    }
    for m in ["express", "lodash", "@nestjs/core", "react-dom", "zod"] {
        assert!(is_third_party_module(m, "src/a.ts"), "{m}");
    }
    // node 内置：不是新第三方依赖，不应触发 HP1
    for m in ["fs", "path", "node:fs", "crypto", "node:crypto", "os", "http"] {
        assert!(!is_third_party_module(m, "src/a.ts"), "内置模块 {m} 不得算第三方");
    }
    // Python 标准库同理
    for m in ["os", "sys", "json", "typing", "dataclasses", "collections"] {
        assert!(!is_third_party_module(m, "src/a.py"), "stdlib {m}");
    }
}

/// 同一 workspace 内的兄弟包名也不能算第三方。
#[test]
fn sibling_workspace_pkg_is_not_third_party() {
    assert!(!is_third_party_module("@acme/shared", "packages/app/src/a.ts"));
    assert!(!is_third_party_module("diffintent-core", "crates/x/src/a.ts"));
}

#[test]
fn test_path_detection() {
    for p in ["src/a.test.ts", "tests/x.py", "spec/a.rb", "src/__tests__/a.ts", "test_a.py", "src/a.spec.ts"] {
        assert!(is_test_path(p), "{p}");
    }
    for p in ["src/a.ts", "src/latest.ts", "attest/x.ts"] {
        assert!(!is_test_path(p), "{p}");
    }
}
```

Run: `cargo test --locked --test detectors_file` → Expected: 编译失败。

- [ ] **Step 3: 框架**

`src/events/detect.rs`：

```rust
//! 检测器框架。spec §4.2：统一签名 + static 数组固定顺序，
//! 不用 trait object——注册序与累加序不得成为不确定性入口。

use crate::evidence::ResolvedFile;
use crate::seir::{Direction, Event, EventKind, EvidenceTier, ScopeRef};

pub struct DetectorCtx<'a> {
    pub file: &'a ResolvedFile,
    /// 仅 full_ast 有值
    pub old: Option<&'a crate::ast::ParsedFile>,
    pub new: Option<&'a crate::ast::ParsedFile>,
}

#[derive(Debug, Clone, Copy)]
pub struct Detector {
    pub name: &'static str,
    pub run: fn(&DetectorCtx, &mut EventSink),
}

#[derive(Debug, Default)]
pub struct EventSink {
    events: Vec<Event>,
}

impl EventSink {
    pub fn push(&mut self, e: Event) {
        self.events.push(e);
    }
    pub fn into_events(self) -> Vec<Event> {
        self.events
    }
}

/// **顺序即规范序的一部分。** 改动本数组顺序会让 --dump-events 输出变化，
/// tests/detectors_file.rs::detector_order_is_frozen 会失败——那是提醒，同步改断言即可。
pub static DETECTORS: &[Detector] = &[
    Detector { name: "file_level", run: super::file_level::detect },
    // Task 9 追加 signature / binding / call
    // Task 10 追加 control_flow / guard / error_handling
    // Task 11 追加 expression / comment / test_case
];

impl EventSink {
    pub fn emit_simple(&mut self, ctx: &DetectorCtx, kind: EventKind, direction: Direction, node_kind: &str, range: (u32, u32), scope: Option<ScopeRef>) {
        self.push(Event {
            file: ctx.file.id,
            kind,
            direction,
            scope,
            node_kind: node_kind.to_string(),
            range,
            tier: ctx.file.tier,
        });
    }
}

/// node 内置模块 + Python stdlib + workspace 兄弟包名。
const BUILTIN_JS: &[&str] = &["assert","async_hooks","buffer","child_process","cluster","console","constants","crypto","dgram","diagnostics_channel","dns","domain","events","fs","http","http2","https","inspector","module","net","os","path","perf_hooks","process","punycode","querystring","readline","repl","stream","string_decoder","sys","timers","tls","trace_events","tty","url","util","v8","vm","wasi","worker_threads","zlib"];
const STDLIB_PY: &[&str] = &["abc","argparse","array","ast","asyncio","base64","bisect","builtins","calendar","collections","copy","csv","ctypes","datetime","decimal","difflib","dataclasses","enum","errno","exceptions","fileinput","fnmatch","fractions","ftplib","functools","gc","getpass","glob","gzip","heapq","hmac","http","importlib","inspect","io","ipaddress","itertools","json","keyword","linecache","locale","logging","math","mimetypes","numbers","operator","os","pathlib","pickle","platform","pprint","profile","pty","queue","random","re","readline","reprlib","secrets","select","shlex","shutil","signal","site","socket","sre_compile","stat","statistics","string","subprocess","sys","tarfile","tempfile","termios","textwrap","threading","time","timeit","token","tokenize","trace","traceback","types","typing","unicodedata","unittest","urllib","uuid","venv","warnings","weakref","webbrowser","xml","zipfile","zipimport","zlib"];

pub fn is_test_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase().replace('\\', "/");
    let name = p.rsplit('/').next().unwrap_or(&p);
    p.contains("/__tests__/") || p.starts_with("tests/") || p.starts_with("test/")
        || p.starts_with("spec/") || p.contains("/tests/")
        || name.ends_with(".test.ts") || name.ends_with(".test.tsx") || name.ends_with(".test.js")
        || name.ends_with(".spec.ts") || name.ends_with(".spec.js")
        || (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
}

/// spec §9.3 HP1 (a)：相对/别名/内置/workspace 兄弟包都不算第三方依赖。
pub fn is_third_party_module(module: &str, importer_path: &str) -> bool {
    let m = module.trim();
    if m.is_empty() { return false; }
    if m.starts_with('.') || m.starts_with('/') { return false; }
    if m.starts_with("@/") || m.starts_with("~/") || m.starts_with("#/") { return false; }
    let bare = m.strip_prefix("node:").unwrap_or(m);
    let root = bare.split(['/', '.']).next().unwrap_or(bare);
    if BUILTIN_JS.contains(&root) || STDLIB_PY.contains(&root) { return false; }
    if importer_path.contains("/packages/") {
        let ws = importer_path.split("/packages/").nth(1).and_then(|s| s.split('/').next());
        if ws.map(|w| format!("@{w}") == root).unwrap_or(false) || Some(root) == ws { return false; }
    }
    if root.ends_with("-core") || root.ends_with("-shared") || root.ends_with("-utils") {
        return false;
    }
    true
}
```

`is_third_party_module` 里的 `-core`/`-shared` 后缀判断是**启发式**，可能把真实的第三方包（如 `react-core`）误判成本地。`tests/detectors_file.rs` 里为它加一条明确失败用例并把限制写进注释，W3 的 HP1 会因此保守（宁可少报 drift，符合 spec §8.2）。**不要在这一步加配置项**（无 CLI flag，spec §2 表面已定）。

- [ ] **Step 4: 文件级检测器**

`src/events/file_level.rs`：

```rust
//! DETECTORS[0..3]：file / dependency / module / config_key。
//! 四类共同点：只看文件与顶层节点，不依赖 definition 配对。

use super::detect::*;
use crate::evidence::is_config_path;
use crate::patch::FileChange;
use crate::seir::{Direction, EventKind};
use tree_sitter::Node;

pub fn file_level(ctx: &DetectorCtx, sink: &mut EventSink) {
    let f = &ctx.file;
    // Deleted 只能从 diff 结构得知（磁盘上已无新侧）。
    if matches!(f.diff.change, FileChange::Deleted) {
        let last = f.diff.hunks.iter().map(|h| h.old_start + h.old_count as u32).max().unwrap_or(1);
        sink.emit_simple(ctx, EventKind::File, Direction::Removed, "file", (1, last), None);
    }
    if matches!(f.diff.change, FileChange::Added) {
        // 只有内含定义才算 module:added（spec §5.1：整新文件**且内含定义**）。
        // 纯新增一个 3 行常量文件不算模块，否则 HP2 会大量误报。
        let has_defs = ctx.new.map(|pf| crate::ast::definitions(pf).len()).unwrap_or(0) > 0;
        if has_defs {
            let last = f.diff.hunks.iter().map(|h| h.new_start + h.new_count as u32).max().unwrap_or(1);
            sink.emit_simple(ctx, EventKind::Module, Direction::Added, "file", (1, last), None);
        } else {
            let last = f.diff.hunks.iter().map(|h| h.new_start + h.new_count as u32).max().unwrap_or(1);
            sink.emit_simple(ctx, EventKind::File, Direction::Added, "file", (1, last), None);
        }
    }
}

/// 顶层 import 的新增/删除。第三方判定交给 is_third_party_module（spec §9.3 HP1）。
pub fn dependency(ctx: &DetectorCtx, sink: &mut EventSink) {
    let (Some(old), Some(new)) = (ctx.old, ctx.new) else {
        // hunk_only 下 import 语句仍可见（是独立 hunk 行），但模块归属不可靠：
        // 只在 full_ast 出 dependency 事件，hunk_only 留空，由 Task 12 显式决定降级形态。
        return;
    };
    let old_imports = top_level_imports(old);
    let new_imports = top_level_imports(new);
    for (line, module) in &new_imports {
        if !old_imports.iter().any(|(_, m)| m == module) {
            let kind_name = "import_statement";
            sink.emit_simple(ctx, EventKind::Dependency, Direction::Added, kind_name, (*line, *line + 1), None);
        }
    }
    for (line, module) in &old_imports {
        if !new_imports.iter().any(|(_, m)| m == module) {
            sink.emit_simple(ctx, EventKind::Dependency, Direction::Removed, "import_statement", (*line, *line + 1), None);
        }
    }
}

fn top_level_imports(pf: &crate::ast::ParsedFile) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut c = pf.root().walk();
    for ch in pf.root().named_children(&mut c) {
        let node = unwrap_import(ch);
        let Some(n) = node else { continue };
        if let Some(m) = module_of(pf, n) {
            out.push((n.start_position().row as u32 + 1, m));
        }
    }
    out.sort();
    out
}

fn unwrap_import<'a>(n: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    match n.kind() {
        "import_statement" | "import_from_statement" => Some(n),
        "export_statement" => {
            let mut c = n.walk();
            n.children(&mut c).find(|ch| ch.kind() == "import_statement")
        }
        "expression_statement" => {
            let mut c = n.walk();
            n.children(&mut c).find(|ch| ch.kind() == "call")
        }
        _ => None,
    }
}

fn module_of(pf: &crate::ast::ParsedFile, n: tree_sitter::Node<'_>) -> Option<String> {
    if let Some(s) = n.child_by_field_name("source") {
        return Some(pf.text_of(s).trim_matches('"').trim_matches('\'').to_string());
    }
    let mut c = n.walk();
    n.descendants(&mut c).find(|d| d.kind() == "string").map(|s| {
        pf.text_of(s).trim_matches('"').trim_matches('\'').to_string()
    })
}

/// 配置文件：不需要 AST（spec §5.1 的 config ↑↑ 按扩展名判定）。
/// hunk_only 与 text_fallback 也能出这一事件——它是本产品里少数
/// 低 tier 也可靠的信号，这个区别要在 W2 的校准分桶里体现。
pub fn config_key(ctx: &DetectorCtx, sink: &mut EventSink) {
    if !is_config_path(ctx.file.diff.newest_path()) { return; }
    for h in &ctx.file.diff.hunks {
        let touched = h.lines.iter().any(|l| matches!(l, crate::patch::DiffLine::Added(_) | crate::patch::DiffLine::Removed(_)));
        if touched {
            sink.emit_simple(ctx, EventKind::ConfigKey, Direction::Modified, "config", (h.new_start, h.new_start + h.new_count as u32), None);
        }
    }
}
```

`DETECTORS` 数组补上三项（`dependency`、`config_key` 各自 `run:`），并同步 Step 2 的 `detector_order_is_frozen` 断言。

- [ ] **Step 5: 接线 extract_events 并跑金标准**

`src/events/mod.rs` 的 `extract_events`：

```rust
pub fn extract_events(files: &[ResolvedFile]) -> Vec<Event> {
    let mut all = Vec::new();
    for f in files {
        // full_ast 才建树；解析失败（语法错误）时降到 hunk_only 并留 reason
        let (old, new) = match (f.old_source.as_deref(), f.new_source.as_deref()) {
            (Some(o), Some(n)) => {
                let lang = f.language.expect("full_ast implies a language");
                match (ParsedFile::parse(lang, o), ParsedFile::parse(lang, n)) {
                    (Ok(ot), Ok(nt)) => (Some(ot), Some(nt)),
                    (Ok(_), Err(e)) => { crate::evidence::note_parse_failed(f, &e); (None, None) }
                    _ => (None, None),
                }
            }
            (None, Some(n)) if matches!(f.diff.change, FileChange::Added) => {
                let lang = f.language.expect("full_ast implies a language");
                (None, ParsedFile::parse(lang, n).ok())
            }
            _ => (None, None),
        };
        let ctx = DetectorCtx { file: f, old: old.as_ref(), new: new.as_ref() };
        let mut sink = EventSink::default();
        for d in detect::DETECTORS {
            (d.run)(&ctx, &mut sink);
        }
        all.extend(sink.into_events());
    }
    canonical_sort(&mut all);
    all
}
```

`note_parse_failed` 需要可变访问而签名是 `&ResolvedFile`——**改用返回值**：`extract_events` 内部收集 `Vec<(FileId, TierReason)>` 并在返回类型里带出，`pub fn extract_events(files: &[ResolvedFile]) -> ExtractOutcome`，`ExtractOutcome { events: Vec<Event>, degradations: Vec<Degradation> }`。**同步改 `tests/golden_events.rs` 用 `.events`。** 这条改动是必要的：tier 是 per-file 事实，AST 解析失败发生在事件抽取阶段，如果不把降级带出来，`summary.evidence_tiers` 就会与 block 的 `evidence_tier` 不一致。

`cargo run -p xtask -- gendev` 生成五个 case 的工作树，填五个 `.events.json`（先跑一次打印实际输出，人工核对是否**正确**再落盘——不要直接把实际值当期望值，那是自证）。

Run: `cargo test --locked` → 全 PASS。

- [ ] **Step 6: 提交**

```bash
git add src/events tests/golden tests/golden_worktree tests/detectors_file.rs tests/golden_events.rs
git commit -m "feat: 检测器框架与文件级事件（file/dependency/module/config_key）

DETECTORS 是 static 数组 + 统一 fn 指针，顺序写死并有断言守卫（spec §4.2）。
module:added 要求新文件内含定义，否则 HP2 会因大量琐碎新文件误报（spec §8.2）。
内置模块/stdlib/workspace 兄弟包都不算第三方依赖；-core/-shared 后缀是已知启发式，
方向上偏保守以少报 drift。
金标准期望值就是 serde 输出，与 --dump-events 同源，不存在第二套渲染。"
```

---

### Task 9: 定义级事件（signature / binding / call）

覆盖 spec §5.1 的 `signature`（子类型分流 `add`/`refactor` 的前提）、`binding`、`call`。三者都建立在 Task 6 的 `pair_definitions` 之上。

**Files:**
- Create: `src/events/scope_diff.rs`, `src/events/signature.rs`
- Modify: `src/events/detect.rs`（DETECTORS 追加）
- Create: `tests/golden/06_sig_optional_param.*`, `07_sig_rename.*`, `08_param_renamed.*`, `09_binding_added.*`, `10_call_changed.*`
- Test: `tests/detectors_signature.rs`, `tests/golden_events.rs`（追加 case）

**Interfaces:**
- Consumes: Task 6 `pair_definitions` `definitions` `enclosing_scope`；Task 8 框架
- Produces:
  - `events::scope_diff::statement_list(Node) -> Vec<Node>` / `align_statements(old: &[Node], new: &[Node]) -> Vec<StmtPair>`
  - `events::signature::SignatureChange { params_added_required, params_added_optional, params_removed, param_renamed, default_changed, return_changed }`（位标志或结构体布尔字段）
  - `events::signature::diff_signature(&ParsedFile, &ParsedFile, old_def: Node, new_def: Node) -> SignatureChange`
  - `events::detect::DETECTORS` 追加 `signature` / `binding` / `call`

- [ ] **Step 1: 写失败测试——签名子类型必须可分**

`tests/detectors_signature.rs`：

```rust
use diffintent::{ast::ParsedFile, events::signature::*, evidence::Language};

fn ts(src: &str) -> ParsedFile { ParsedFile::parse(Language::TypeScript, src).unwrap() }
fn first_def(pf: &ParsedFile) -> tree_sitter::Node<'_> {
    diffintent::ast::definitions(pf).remove(0).1
}

/// 新增**必填**参数 → 调用方必须改，是 add 的强信号。
#[test]
fn added_required_param_is_flagged() {
    let o = ts("function f(a: number) { return a; }");
    let n = ts("function f(a: number, b: string) { return a + b; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.params_added_required && !c.params_added_optional, "{c:?}");
}

/// 新增**可选**参数不破坏调用方，add 的先验弱于必填。
#[test]
fn added_optional_param_is_distinguished() {
    let o = ts("function f(a: number) { return a; }");
    let n = ts("function f(a: number, b?: string) { return a; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.params_added_optional && !c.params_added_required, "{c:?}");
}

#[test]
fn added_default_param_is_optional_flavored() {
    let o = ts("function f(a: number) { return a; }");
    let n = ts("function f(a: number, b = 2) { return a; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.params_added_optional && !c.params_added_required, "{c:?}");
}

/// 只改默认值 = 无新参数的调整 → refactor 先验（spec §5.1 第二行）。
#[test]
fn default_only_change_is_not_a_new_param() {
    let o = ts("function f(a = 1) { return a; }");
    let n = ts("function f(a = 2) { return a; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.default_changed && !c.params_added_required && !c.params_added_optional, "{c:?}");
}

#[test]
fn param_rename_detected() {
    let o = ts("function f(value: number) { return value; }");
    let n = ts("function f(v: number) { return v; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.param_renamed && !c.params_added_required && !c.params_removed, "{c:?}");
}

#[test]
fn return_type_change_detected() {
    let o = ts("function f(): number { return 1; }");
    let n = ts("function f(): string { return \"1\"; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.return_changed && !c.param_renamed, "{c:?}");
}

#[test]
fn removed_param_detected() {
    let o = ts("function f(a: number, b: string) { return a; }");
    let n = ts("function f(a: number) { return a; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.params_removed, "{c:?}");
}

/// 完全没动签名 → 全 false。这条挡住"任何文本差异都算签名变化"的偷懒实现。
#[test]
fn body_only_change_has_no_signature_change() {
    let o = ts("function f(a: number) { return a; }");
    let n = ts("function f(a: number) { return a + 1; }");
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert_eq!(c, SignatureChange::default(), "{c:?}");
}

/// Python 的参数形态（*args / **kw / 带默认值 / 类型注解）必须同样可分。
#[test]
fn python_params_are_classified() {
    let o = ParsedFile::parse(Language::Python, "def f(a):\n    return a\n").unwrap();
    let n = ParsedFile::parse(Language::Python, "def f(a, b=None):\n    return a\n").unwrap();
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert!(c.params_added_optional && !c.params_added_required, "{c:?}");

    let n2 = ParsedFile::parse(Language::Python, "def f(a, b, *args, **kw):\n    return a\n").unwrap();
    let c2 = diff_signature(&o, &n2, first_def(&o), first_def(&n2));
    assert!(c2.params_added_required, "{c2:?}");
}

/// 装饰器变化不得被当成签名变化（它是另一回事，W1 不出事件）。
#[test]
fn decorator_change_is_not_signature() {
    let o = ParsedFile::parse(Language::Python, "@staticmethod\ndef f(a):\n    return a\n").unwrap();
    let n = ParsedFile::parse(Language::Python, "@classmethod\ndef f(a):\n    return a\n").unwrap();
    let c = diff_signature(&o, &n, first_def(&o), first_def(&n));
    assert_eq!(c, SignatureChange::default(), "{c:?}");
}

/// binding / call 的事件只在**已配对**的定义内部产生，
/// 否则一次"删掉整个函数"会同时报 file/module + 一堆 binding:removed，噪声淹没信号。
#[test]
fn removed_definition_does_not_emit_inner_binding_noise() {
    let e = events_ts(
        "function a() { const x = 1; use(x); }\nfunction b() {}\n",
        "function b() {}\n",
    );
    assert!(!e.iter().any(|x| x.kind == EventKind::Binding && x.direction == Direction::Removed),
        "被整体删除的定义不得逐条报 binding:removed: {e:?}");
}
```

**本任务同时把 `events_ts` / `events_py` / `events_ts_at` / `events_py_at` 加进 `tests/common/mod.rs`**（Task 10/11 的测试也用它，故必须在此实现完整）：

```rust
/// 把 old/new 两段源码写进真 git 仓，跑完整流水线，返回事件。
/// 默认路径 `src/m.ts`；`*_at` 变体允许指定路径（test 路径判定依赖它）。
pub fn events_ts(old: &str, new: &str) -> Vec<diffintent::seir::Event> {
    events_ts_at("src/m.ts", old, new)
}

pub fn events_ts_at(path: &str, old: &str, new: &str) -> Vec<diffintent::seir::Event> {
    events_at(Language::TypeScript, path, old, new)
}

pub fn events_py(old: &str, new: &str) -> Vec<diffintent::seir::Event> {
    events_py_at("src/m.py", old, new)
}

pub fn events_py_at(path: &str, old: &str, new: &str) -> Vec<diffintent::seir::Event> {
    events_at(Language::Python, path, old, new)
}

fn events_at(lang: Language, path: &str, old: &str, new: &str) -> Vec<diffintent::seir::Event> {
    let ext_old = if lang.ext() == "python" { "old.py" } else { "old.ts" };
    let (_dir, diff) = real_repo(&[(path, old)], &[(path, new)]);
    let _ = ext_old;
    let wt = Worktree::new(Some(_dir.clone())).unwrap();
    let a = diffintent::analyze(&diff, Some(&wt), None).expect("analyze");
    // 前提断言：这些测试全部要求 full_ast，否则"没产出事件"可能只是因为降级了
    for r in &a.resolved {
        assert_eq!(r.tier, EvidenceTier::FullAst,
            "{} 未达 full_ast（{:?}）：real_repo 产物与 diff 漂移", r.diff.newest_path(), r.reason);
    }
    a.events
}

/// 无工作树跑同一份 diff —— Task 12 用它证明低 tier 确实收窄了能力。
pub fn events_hunk_only(new: &str) -> Vec<diffintent::seir::Event> {
    let (_dir, diff) = real_repo(&[("src/m.ts", "placeholder\n")], &[("src/m.ts", new)]);
    let a = diffintent::analyze(&diff, None, None).expect("analyze");
    a.events
}
```

`use diffintent::{evidence::{Language, Worktree}, seir::EvidenceTier};` 补进 common 的 import；`Language` 需 `pub fn ext()`（Task 5 已定义）。`events_at` 里 `ext_old` 无用，删掉它。`real_repo` 的 before 只放一个占位文件的用法不适用于 `events_hunk_only`——那里 before/after 都要真内容，按 `real_repo(&[(path, old)], &[(path, new)])` 的形态传，`events_hunk_only` 改成接 `(old, new)` 两个参数并同步改 Task 10 的调用点。

`events_for` 在 Step 5 用 `tests/golden_events.rs` 里的公共构造器实现（把 `run_case` 拆成 `run_case_with_sources(path, old, new)`，供所有测试复用；`golden_events.rs` 与 `detectors_*.rs` 都 `mod common;` 引入）。

- [ ] **Step 2: 语句对齐骨架**

`src/events/scope_diff.rs`：

```rust
//! definition 体内的 statement-list 对齐。Task 10/11 的 control_flow / guard /
//! error_handling / expression 全部依赖它，因此**必须在这里一次做对**。
//! 对齐用"token 流相同即同一条语句"的保守规则：位置错位的语句宁可报成
//! 删+加（两条事件），也不要错配成"同一条语句被修改"（一条假 expression 事件）。

use crate::ast::ParsedFile;
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmtPair {
    /// 空白归一后文本相同
    Same { old: usize, new: usize },
    /// 结构骨架相同（kind 与 named_child_count 都相同）但文本不同 → expression:modified 的输入
    Loose { old: usize, new: usize },
    Added { new: usize },
    Removed { old: usize },
}

/// 对齐所需的最小语句信息。**故意不带 Node**：Node 有生命周期，跨函数传递 Vec<Node>
/// 会让借用检查把调用方绕死；text/kind/child_count 三样足够表达判据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StmtInfo {
    pub text: String,
    pub kind: String,
    pub named_children: usize,
    pub range: (u32, u32),
}

/// 取一个 definition 体的顶层语句信息（不含 `{` `}` 本身）。
pub fn statement_list(pf: &ParsedFile, def: Node<'_>) -> Vec<StmtInfo> {
    let body = def.child_by_field_name("body")
        .or_else(|| {
            let mut c = def.walk();
            def.children(&mut c).find(|ch| matches!(ch.kind(), "statement_block" | "block"))
        });
    let Some(body) = body else { return Vec::new(); };
    let mut c = body.walk();
    body.named_children(&mut c).map(|n| StmtInfo {
        text: pf.text_of(n).to_string(),
        kind: n.kind().to_string(),
        named_children: n.named_child_count(),
        range: (n.start_position().row as u32 + 1, n.end_position().row as u32 + 2),
    }).collect()
}

pub fn align_statements(old: &[StmtInfo], new: &[StmtInfo]) -> Vec<StmtPair> {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let an: Vec<String> = old.iter().map(|o| norm(&o.text)).collect();
    let bn: Vec<String> = new.iter().map(|o| norm(&o.text)).collect();
    let m = an.len();
    let n = bn.len();
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i][j] = if an[i] == bn[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < m && j < n {
        if an[i] == bn[j] {
            out.push(StmtPair::Same { old: i, new: j });
            i += 1; j += 1;
        } else if same_skeleton(&old[i], &new[j]) {
            // 骨架相同、文本不同：这是一条被修改的语句，不是删+加。
            // LCS 里把它当匹配点，否则一次 `return a` → `return a + 1` 会碎成两条。
            out.push(StmtPair::Loose { old: i, new: j });
            i += 1; j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(StmtPair::Removed { old: i }); i += 1;
        } else {
            out.push(StmtPair::Added { new: j }); j += 1;
        }
    }
    while i < m { out.push(StmtPair::Removed { old: i }); i += 1; }
    while j < n { out.push(StmtPair::Added { new: j }); j += 1; }
    out
}

fn same_skeleton(a: &StmtInfo, b: &StmtInfo) -> bool {
    a.kind == b.kind && a.named_children == b.named_children
}
```

`Loose` 参与 LCS 的匹配点（上面 `else if same_skeleton(...)` 分支），但 `dp` 表是按**文本相等**建的，`Loose` 在回溯时才判定——这会让 dp 与实际路径不一致。**落地必须把 dp 的相等判据改成 `an[i] == bn[j] || same_skeleton(&old[i], &new[j])`**，回溯时再区分 `Same` 与 `Loose`。上面的写法是说明性的，实现时按这个修正写，并由 `tests/ast.rs` 之外新增的 `tests/scope_diff.rs` 钉住三种场景：纯文本相等→`Same`、`return a`→`return a + 1`→`Loose`、`if (x) {…}`→`for (…)`→`Removed`+`Added`（骨架不同，绝不 `Loose`）。

对齐用"骨架相同即同一条语句"的保守规则：`kind` 或 `named_child_count` 不同就宁可报成删+加（两条事件），也不要错配成"同一条语句被修改"（一条假 `expression` 事件）。

- [ ] **Step 3: 签名差分**

`src/events/signature.rs`：

```rust
//! spec §5.1 第一/二行：新增参数（add ↑）与"重命名/默认值调整，无新参数"（refactor ↑）
//! 必须可分，否则 add 与 refactor 无法区分。

use crate::ast::ParsedFile;
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureChange {
    pub params_added_required: bool,
    pub params_added_optional: bool,
    pub params_removed: bool,
    pub param_renamed: bool,
    pub default_changed: bool,
    pub return_changed: bool,
}

impl SignatureChange {
    pub fn is_any(&self) -> bool {
        [self.params_added_required, self.params_added_optional, self.params_removed,
         self.param_renamed, self.default_changed, self.return_changed].iter().any(|x| *x)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Param {
    name: String,
    has_default: bool,
    optional: bool,   // `b?: T` 或 `*args` / `**kw`
    type_text: String,
}

pub fn diff_signature(old: &ParsedFile, new: &ParsedFile, old_def: Node<'_>, new_def: Node<'_>) -> SignatureChange {
    let po = params_of(old, old_def);
    let pn = params_of(new, new_def);
    let mut c = SignatureChange::default();

    // 参数名多重集差异
    let mut only_new: Vec<&Param> = Vec::new();
    let mut only_old: Vec<&Param> = Vec::new();
    let mut used = vec![false; po.len()];
    for p in &pn {
        if let Some(i) = po.iter().enumerate().find(|(_, q)| q.name == p.name).map(|(i, _)| i) {
            if !used[i] { used[i] = true; continue; }
        }
        only_new.push(p);
    }
    for (i, q) in po.iter().enumerate() {
        if !used[i] { only_old.push(q); }
    }
    // 数量与"位置"都相同但名字不同 → 改名，不是增删
    if only_old.len() == 1 && only_new.len() == 1 && po.len() == pn.len() {
        c.param_renamed = true;
        only_old.clear();
        only_new.clear();
    }
    for p in only_new {
        if p.optional || p.has_default { c.params_added_optional = true } else { c.params_added_required = true }
    }
    if !only_old.is_empty() { c.params_removed = true; }

    // 同名参数的默认值/类型变化
    for p in &pn {
        if let Some(q) = po.iter().find(|q| q.name == p.name) {
            if q.has_default != p.has_default || q.type_text != p.type_text {
                // 类型文本变化只在**非改名**时算 default_changed 语义之外的信号；
                // 严格区分需要类型语义，W1 不做（spec §5.1 推迟项）
                if q.has_default != p.has_default || (!q.type_text.is_empty() && !p.type_text.is_empty() && q.type_text != p.type_text) {
                    c.default_changed = true;
                }
            }
        }
    }
    if return_text(old, old_def) != return_text(new, new_def) {
        c.return_changed = true;
    }
    c
}

fn params_of(pf: &ParsedFile, def: Node<'_>) -> Vec<Param> {
    let Some(list) = def.child_by_field_name("parameters")
        .or_else(|| { let mut c = def.walk(); def.children(&mut c).find(|n| matches!(n.kind(), "parameters" | "formal_parameters")) })
    else { return Vec::new(); };
    let mut out = Vec::new();
    let mut c = list.walk();
    for n in list.named_children(&mut c) {
        match n.kind() {
            // TS/JS
            "required_parameter" | "optional_parameter" => {
                let pat = n.child_by_field_name("pattern").unwrap_or(n);
                let name = leaf_identifier(pf, pat).unwrap_or_default();
                out.push(Param {
                    name,
                    has_default: n.child_by_field_name("value").is_some(),
                    optional: n.kind() == "optional_parameter" || n.child_by_field_name("value").is_some(),
                    type_text: n.child_by_field_name("type").map(|t| pf.text_of(t).to_string()).unwrap_or_default(),
                });
            }
            // Python
            "identifier" => out.push(Param { name: pf.text_of(n).to_string(), has_default: false, optional: false, type_text: String::new() }),
            "default_parameter" | "typed_default_parameter" => {
                out.push(Param {
                    name: n.child_by_field_name("name").map(|x| pf.text_of(x).to_string()).unwrap_or_default(),
                    has_default: true, optional: true,
                    type_text: n.child_by_field_name("type").map(|t| pf.text_of(t).to_string()).unwrap_or_default(),
                });
            }
            "list_splat_pattern" | "dictionary_splat_pattern" | "tuple_pattern" => {
                out.push(Param { name: String::new(), has_default: false, optional: true, type_text: String::new() });
            }
            "parameter" => { /* Python 带注解的形参 */
                out.push(Param {
                    name: leaf_identifier(pf, n).unwrap_or_default(),
                    has_default: false,
                    optional: false,
                    type_text: n.child_by_field_name("type").map(|t| pf.text_of(t).to_string()).unwrap_or_default(),
                });
            }
            _ => {}
        }
    }
    out
}

fn leaf_identifier(pf: &ParsedFile, n: Node<'_>) -> Option<String> {
    if n.kind() == "identifier" || n.kind() == "type_identifier" {
        return Some(pf.text_of(n).to_string());
    }
    let mut c = n.walk();
    n.descendants(&mut c).find(|d| d.kind() == "identifier").map(|d| pf.text_of(d).to_string())
}

/// 返回类型文本。TS 用 `type` field；Python 的 `-> T` 在 grammar 里是 `type` field。
fn return_text(pf: &ParsedFile, def: Node<'_>) -> String {
    def.child_by_field_name("return_type")
        .or_else(|| def.child_by_field_name("type"))
        .map(|t| pf.text_of(t).to_string())
        .unwrap_or_default()
}
```

`params_of` 的 kind 分支以 Task 6 的 kinds.tsv 实测为准；上面列的 `required_parameter` / `optional_parameter` / `default_parameter` / `typed_default_parameter` / `list_splat_pattern` / `dictionary_splat_pattern` / `parameter` 都必须在 tsv 里存在，否则该分支是死代码——**Step 1 的 `python_params_are_classified` 会立刻暴露**。`is_any` 若无调用方就删掉（clippy `-D warnings`）。

- [ ] **Step 4: binding / call 检测器 + DETECTORS 追加**

`src/events/signature.rs` 追加三个检测器函数（`signature` / `binding` / `call`），共享同一份 `pair_definitions` 结果。**同一文件内配对只做一次**——三个检测器各自重算配对是浪费且会产生不一致，做法是 `DETECTORS` 的这三项改挂在一个复合检测器 `definition_level` 下，或让 `DetectorCtx` 带一个 `OnceCell<Vec<Pair>>`。**决定：合并成一个检测器 `definition_level`，`DETECTORS` 从 3 项变 1 项**，同步改 Step 1 的顺序断言。理由：三个检测器共享同一份配对，分开注册只是把一次计算变三次，并给不确定性留三个入口。

- [ ] **Step 5: 跑测试并落金标准**

```bash
cargo test --locked
cargo run -p xtask -- gendev
```

Expected: Step 1 全 PASS；`removed_definition_does_not_emit_inner_binding_noise` 逼出"只对**已配对**定义做内部差分，未配对定义只出一条 `signature`/`file` 级事件"的实现。

金标准六个 case 落盘前先人工核对：`06_sig_optional_param` 期望恰一条 `signature:modified`，`07_sig_rename` 一条，`08_param_renamed` 一条，`09_binding_added` 一条 `binding:added`，`10_call_changed` 一条 `call:modified`。若某 case 产出多条，先判断哪条是多余的，**不要通过放宽期望来放行**。

- [ ] **Step 6: 提交**

```bash
git add src/events tests/detectors_signature.rs tests/golden tests/golden_worktree
git commit -m "feat: 定义级事件（signature/binding/call）与语句对齐

签名子类型必须可分：新增必填/可选/默认值参数与"仅改默认值"给出不同信号，
否则 add 与 refactor 无法区分（spec §5.1）。
语句对齐用 LCS + 空白归一；宁可报删+加也不错配成假的 expression:modified。
三个检测器合并为 definition_level：共享一次 pair_definitions，
不给不确定性留三个入口。"
```

---

### Task 10: 语句级事件（control_flow / guard / error_handling）

覆盖 spec §5.1 里 `fix ↑↑` 与"Removed 单独成事"两条。**`error_handling:removed` 是 HP3 的唯一输入**（spec §9.3 里无条件报 drift 的那条），方向丢失 = drift 能力丢失。

**Files:**
- Create: `src/events/statements.rs`
- Modify: `src/events/detect.rs`
- Create: `tests/golden/11_guard_added.*`, `12_try_removed.*`, `13_new_branch_no_guard.*`, `14_async_added.*`
- Test: `tests/detectors_statements.rs`, `tests/golden_events.rs`

**Interfaces:**
- Consumes: Task 9 `align_statements`；Task 6 `kind_is_guard` `kind_is_control_flow` `kind_is_error_handling`
- Produces: `events::statements::detect(ctx, sink)`（一个检测器出三类事件）、`events::statements::guard_nodes(&ParsedFile, Node) -> Vec<Node>`

- [ ] **Step 1: 写失败测试**

`tests/detectors_statements.rs`：

```rust
use diffintent::seir::{Direction, EventKind};
use diffintent_test_support::*;

/// guard vs control_flow 的分界是 spec §5.1 里 `fix ↑↑` 的精度来源。
#[test]
fn early_return_is_guard_not_control_flow() {
    let e = events_ts(
        "function f(a) {\n  return a;\n}",
        "function f(a) {\n  if (!a) return;\n  return a;\n}",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::Guard && x.direction == Direction::Added), "{e:?}");
    assert!(!e.iter().any(|x| x.kind == EventKind::ControlFlow), "早退 if 只报 guard，不重复报 control_flow");
}

/// 带副作用的 if 是 control_flow，不是 guard。
#[test]
fn if_with_side_effect_is_control_flow_only() {
    let e = events_ts(
        "function f(a) {\n  return a;\n}",
        "function f(a) {\n  if (a > 1) { doThing(); }\n  return a;\n}",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::ControlFlow && x.direction == Direction::Added), "{e:?}");
    assert!(!e.iter().any(|x| x.kind == EventKind::Guard), "{e:?}");
}

/// assert 是 guard。Python 的 `assert x` 与 TS 的 `if (!x) throw` 必须同先验。
#[test]
fn python_assert_is_guard() {
    let e = events_py(
        "def f(a):\n    return a\n",
        "def f(a):\n    assert a\n    return a\n",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::Guard && x.direction == Direction::Added), "{e:?}");
}

/// **方向不可丢**：删掉 try/except 必须报 Removed，它是 HP3 的唯一输入。
#[test]
fn removed_try_is_error_handling_removed() {
    let e = events_py(
        "def f():\n    try:\n        go()\n    except ValueError:\n        pass\n",
        "def f():\n    go()\n",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::ErrorHandling && x.direction == Direction::Removed), "{e:?}");
    assert!(!e.iter().any(|x| x.direction == Direction::Added), "不得同时报 added: {e:?}");
}

#[test]
fn added_throw_is_error_handling_added() {
    let e = events_ts(
        "function f() { return 1; }",
        "function f() { if (!x) throw new Error(\"bad\"); return 1; }",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::ErrorHandling && x.direction == Direction::Added), "{e:?}");
}

/// 新增 try 包住的既有语句不得让内部语句全报成 removed+added。
/// 这是本任务最容易做坏的一条：包装层会让整块 body 文本变化。
#[test]
fn wrapping_in_try_does_not_churn_inner_statements() {
    let e = events_py(
        "def f():\n    a()\n    b()\n",
        "def f():\n    try:\n        a()\n        b()\n    except E:\n        pass\n",
    );
    let churn = e.iter().filter(|x| x.kind == EventKind::Expression).count();
    assert_eq!(churn, 0, "被 try 包住的既有语句不得报 expression: {e:?}");
    assert!(e.iter().any(|x| x.kind == EventKind::ErrorHandling && x.direction == Direction::Added));
}

/// async/await 的引入是结构变化，spec §5.1 没有单独 kind——
/// 按 control_flow 报，并让 reason 层（W2）从 node_kind 看出来。**不得新增 kind**。
#[test]
fn async_change_falls_into_control_flow_without_new_kind() {
    let e = events_ts(
        "async function f() { g(); }",
        "async function f() { await g(); }",
    );
    for x in &e {
        assert!(ALL_KINDS_14.contains(&x.kind), "出现非法 kind: {:?}", x.kind);
    }
    assert!(!e.is_empty(), "至少要有一条: {e:?}");
}

/// guard 出现在函数中间（非入口）仍是 guard，但 reason 要能看出来。
#[test]
fn mid_function_guard_still_guard() {
    let e = events_ts(
        "function f(a) { const x = a; return x; }",
        "function f(a) { const x = a; if (!x) return null; return x; }",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::Guard), "{e:?}");
}

/// 删除 if 分支必须报 control_flow:removed，且不能与 guard:removed 同时出。
#[test]
fn removed_guard_is_guard_removed() {
    let e = events_ts(
        "function f(a) {\n  if (!a) return;\n  return a;\n}",
        "function f(a) {\n  return a;\n}",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::Guard && x.direction == Direction::Removed), "{e:?}");
    assert!(!e.iter().any(|x| x.kind == EventKind::ControlFlow), "{e:?}");
}

/// hunk_only 的文件不得产出 guard/control_flow 事件（片段里没有完整函数体）。
/// spec §6：低 tier 不产出只有高 tier 才可靠的信号。
#[test]
fn hunk_only_emits_no_statement_level_events() {
    let e = events_hunk_only("function f(a) {\n  if (!a) return;\n  return a;\n}");
    assert!(e.iter().all(|x| x.kind != EventKind::Guard && x.kind != EventKind::ControlFlow && x.kind != EventKind::ErrorHandling), "{e:?}");
}
```

`diffintent_test_support` 是 `tests/common/mod.rs` 扩展出的 `events_ts(old, new)` / `events_py(old, new)` / `events_hunk_only(new)` / `ALL_KINDS_14`（后者直接复用 `diffintent::EVENT_KIND_NAMES` 派生）。`events_ts` 内部把 `old`/`new` 写进一个临时目录，按 new 侧生成后镜像一致的 diff（用真 `git`，与 Task 4 的 `real_repo` 同一套）。

Run: `cargo test --locked --test detectors_statements` → Expected: 编译失败。

- [ ] **Step 2: 检测器**

`src/events/statements.rs`：

```rust
//! spec §5.1：`fix ↑↑` 与 `error_handling:removed` 单独成事。
//! 一个检测器同时出 guard / control_flow / error_handling——三者共享同一份
//! statement 对齐结果，拆开注册只会在三处给出不一致的分歧点。

use super::detect::*;
use super::scope_diff::{align_statements, statement_list, StmtInfo, StmtPair};
use crate::ast::{kind_is_control_flow, kind_is_error_handling, kind_is_guard, ParsedFile};
use crate::seir::{Direction, EventKind};
use tree_sitter::Node;

pub fn detect(ctx: &DetectorCtx, sink: &mut EventSink) {
    let (Some(old), Some(new)) = (ctx.old, ctx.new) else { return };
    let old_defs = crate::ast::definitions(old);
    let new_defs = crate::ast::definitions(new);
    let pairs = crate::ast::pair::pair_definitions(
        &old_defs.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
        &new_defs.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
    );
    for p in pairs {
        let (oi, ni) = match p { crate::ast::pair::Pair::Matched { old, new } => (old, new), _ => continue };
        let scope = old_defs[oi].0.clone();
        let osl = statement_list(old, old_defs[oi].1);
        let nsl = statement_list(new, new_defs[ni].1);
        for pair in align_statements(&osl, &nsl) {
            match pair {
                StmtPair::Removed { old: a } => classify_and_emit(ctx, sink, &scope, &osl[a], Direction::Removed),
                StmtPair::Added { new: b } => classify_and_emit(ctx, sink, &scope, &nsl[b], Direction::Added),
                // 骨架相同但文本不同：guard/control_flow/error_handling 的种类没变，
                // 所以这里**不出**语句级事件，交给 Task 11 的 expression。
                // 若一侧是 `if` 一侧是裸语句，align 已判为 Removed+Added，会走到上面两支。
                StmtPair::Same { .. } | StmtPair::Loose { .. } => {}
            }
        }
    }
    // 包装层（既有语句被新的 try/if 包住）时，内层语句在 align 里是 Same，
    // 只有外层包装节点出现在 Added 里 —— 这正是想要的行为，由
    // tests/detectors_statements.rs::wrapping_in_try_does_not_churn_inner_statements 钉住。
}

fn classify_and_emit(ctx: &DetectorCtx, sink: &mut EventSink, scope: &crate::seir::ScopeRef, s: &StmtInfo, dir: Direction) {
    if is_guard_info(s) {
        sink.emit_simple(ctx, EventKind::Guard, dir, &s.kind, s.range, Some(scope.clone()));
        return;   // guard 与 control_flow 互斥，不重复报
    }
    if kind_is_error_handling(&s.kind) {
        sink.emit_simple(ctx, EventKind::ErrorHandling, dir, &s.kind, s.range, Some(scope.clone()));
        return;
    }
    if kind_is_control_flow(&s.kind) {
        sink.emit_simple(ctx, EventKind::ControlFlow, dir, &s.kind, s.range, Some(scope.clone()));
    }
}

/// guard 判定需要看 consequent，而 `StmtInfo` 没带 Node。
/// **两条出路，选前者**：
///  (a) 给 `StmtInfo` 加 `early_exit_only: bool`，在 `statement_list` 里用
///      `kind_is_guard(n)` 当场算出（`statement_list` 手里就有 Node）。
///  (b) 让 `classify_and_emit` 重新拿 Node。
/// (a) 更干净且避免二次遍历；`scope_diff.rs` 与 Task 9 的测试一起改。
fn is_guard_info(s: &StmtInfo) -> bool {
    s.kind == "assert_statement" || (s.kind == "if_statement" && s.early_exit_only)
}
```

`StmtInfo` 需要 `early_exit_only: bool` 字段（`is_guard_info` 依赖它），在 `statement_list` 里用 `crate::ast::kind_is_guard(n)` 当场算出——那里手里就有 `Node`。这个改动同时触及 Task 9 的 `scope_diff.rs` 与 `tests/scope_diff.rs`，**必须与 Task 10 在同一次提交里完成**。`kind_is_guard` 的签名是 `fn(Node) -> bool`（Task 6 已定），不接 `&str`。

- [ ] **Step 3: 跑测试，逐条核对**

```bash
cargo test --locked --test detectors_statements
```

`wrapping_in_try_does_not_churn_inner_statements` 是本任务的真正考点。若它挂：说明 `align` 没有把内层 `a()` / `b()` 认成 `Same`。修法是提高 `norm` 的归一强度（去掉所有空白）而**不是**放宽测试——但要确认放宽后 `quote_style_change_is_not_formatting`（Task 7）仍不通过，两条测试构成一组钳形约束。

- [ ] **Step 4: 提交**

```bash
git add src/events tests/detectors_statements.rs tests/golden tests/golden_worktree
git commit -m "feat: 语句级事件（guard/control_flow/error_handling）

guard 与 control_flow 互斥：早退 if 只报 guard，这是 fix ↑↑ 先验精度的来源（spec §5.1）。
error_handling:removed 是 HP3 的唯一输入，方向丢失即 drift 能力丢失（spec §9.3）。
新增 try 包住既有语句不得让内层报 expression churn —— 由 align 的 Same 保证。
hunk_only 不产出只有 full_ast 才可靠的语句级信号（spec §6）。"
```

---

### Task 11: 叶子事件（expression / comment / test_case / formatting）

覆盖 spec §5.1 剩下的 kind，以及 §5.2 的 `formatting` / `comment` 折叠。

**Files:**
- Create: `src/events/leaves.rs`
- Modify: `src/events/detect.rs`
- Create: `tests/golden/15_expr_only.*`, `16_formatting_only.*`, `17_comment_only.*`, `18_test_added.*`
- Test: `tests/detectors_leaves.rs`

**Interfaces:**
- Consumes: 前四任务全部
- Produces: `events::leaves::detect(ctx, sink)`；`DETECTORS` 至此包含 5 项（`file_level`、`dependency`、`config_key`、`definition_level`、`statement_level`、`leaf_level`）——实际 6 项，顺序冻结

- [ ] **Step 1: 写失败测试**

`tests/detectors_leaves.rs`：

```rust
use diffintent::seir::{Direction, EventKind};
use diffintent_test_support::*;

/// 纯表达式微调 → 只报 expression，不报 control_flow。
#[test]
fn expression_only() {
    let e = events_ts(
        "function f(a) { return a + 1; }",
        "function f(a) { return a + 2; }",
    );
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(e[0].kind, EventKind::Expression);
    assert_eq!(e[0].direction, Direction::Modified);
}

/// 纯格式化扰动必须**零事件**（spec §3.1：折叠，不产生意图信号）。
/// 这是 I6 负对照的内核，也是"判据能变红"的那半边。
#[test]
fn formatting_only_yields_zero_events() {
    let e = events_ts(
        "function f(a) {\n  return a;\n}\n",
        "function f ( a ) {\n\n  return a\n\n}\n",
    );
    assert!(e.iter().all(|x| x.kind == EventKind::Formatting), "{e:?}");
    // 更进一步：formatting 是排除层，extract_events 应当把它折叠掉而非输出
    let meaningful: Vec<_> = e.iter().filter(|x| x.kind != EventKind::Formatting).collect();
    assert!(meaningful.is_empty(), "格式化扰动不得产生有意义信号: {e:?}");
}

#[test]
fn comment_only_yields_comment_event() {
    let e = events_ts(
        "function f() { return 1; }\n",
        "// why\nfunction f() { return 1; }\n",
    );
    assert!(e.iter().all(|x| x.kind == EventKind::Comment), "{e:?}");
    assert!(!e.is_empty(), "注释变化至少要有一条 comment");
}

/// comment 与 expression 同时变时两条都要报，但 comment 不参与意图判断（W2）。
#[test]
fn comment_and_expr_both_reported() {
    let e = events_ts(
        "function f(a) { return a; }\n",
        "// note\nfunction f(a) { return a + 1; }\n",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::Comment), "{e:?}");
    assert!(e.iter().any(|x| x.kind == EventKind::Expression), "{e:?}");
}

/// 测试文件里新增用例 → test_case:added。
#[test]
fn test_case_added_in_test_file() {
    let e = events_ts_at(
        "src/a.test.ts",
        "describe(\"x\", () => { it(\"one\", () => { expect(1).toBe(1); }); });\n",
        "describe(\"x\", () => { it(\"one\", () => { expect(1).toBe(1); }); it(\"two\", () => { expect(2).toBe(2); }); });\n",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::TestCase && x.direction == Direction::Added), "{e:?}");
}

#[test]
fn python_test_function_added() {
    let e = events_py_at(
        "tests/test_a.py",
        "def test_ok():\n    assert True\n",
        "def test_ok():\n    assert True\n\ndef test_two():\n    assert 1 == 1\n",
    );
    assert!(e.iter().any(|x| x.kind == EventKind::TestCase && x.direction == Direction::Added), "{e:?}");
}

/// 非测试文件里的 `def test_x` 不得报 test_case（避免把集成脚本当测试）。
#[test]
fn non_test_path_emits_binding_not_testcase() {
    let e = events_py_at(
        "src/helpers.py",
        "def helper():\n    pass\n",
        "def helper():\n    pass\n\ndef test_only_here():\n    pass\n",
    );
    assert!(!e.iter().any(|x| x.kind == EventKind::TestCase), "{e:?}");
}

/// **一条 diff 不得同时是 formatting 和 expression**。
/// 若两个检测器都开火，说明折叠没有挡住后续分类。
#[test]
fn formatting_and_expression_are_mutually_exclusive() {
    let e = events_ts("function f(a){return a;}\n", "function f ( a ) { return a }\n");
    let kinds: Vec<_> = e.iter().map(|x| x.kind).collect();
    assert!(!(kinds.contains(&EventKind::Formatting) && kinds.contains(&EventKind::Expression)), "{kinds:?}");
}

/// 只有注释与格式同时变：comment 一条，formatting 折叠掉。
#[test]
fn comment_plus_formatting() {
    let e = events_ts(
        "function f(a){return a;}\n",
        "// n\nfunction f ( a ) { return a }\n",
    );
    assert!(e.iter().all(|x| x.kind == EventKind::Comment), "{e:?}");
}

/// 一个空行的增删不得产生任何有意义事件。
#[test]
fn blank_line_only_yields_nothing_meaningful() {
    let e = events_ts("const a = 1;\nconst b = 2;\n", "const a = 1;\n\nconst b = 2;\n");
    assert!(e.iter().all(|x| x.kind == EventKind::Formatting), "{e:?}");
}
```

Run: `cargo test --locked --test detectors_leaves` → Expected: 编译失败。

- [ ] **Step 2: 折叠优先于分类**

`src/events/leaves.rs`：

```rust
//! spec §5.2：formatting 与 comment 是排除层。
//! **折叠必须发生在分类之前**——否则同一处变更会同时是 formatting 和 expression，
//! W2 的先验表会收到自相矛盾的输入（Task 11 Step 1 的互斥测试就为此存在）。

use super::detect::*;
use super::lex::{comment_texts, is_formatting_only};
use crate::ast::ParsedFile;
use crate::seir::{Direction, EventKind, EvidenceTier};
use tree_sitter::Node;

pub fn detect(ctx: &DetectorCtx, sink: &mut EventSink) {
    let (Some(old), Some(new)) = (ctx.old, ctx.new) else { return };

    // 1) 全文件级折叠：token 流一致 → 只出 Formatting，并立刻返回
    if is_formatting_only(old, new) {
        sink.emit_simple(ctx, EventKind::Formatting, Direction::Modified, "file", (1, 1), None);
        return;
    }

    // 2) comment 差分（顺序无关，集合比对）
    let co = comment_texts(old);
    let cn = comment_texts(new);
    if co != cn {
        let mut seen = std::collections::BTreeSet::new();
        for s in cn.iter().filter(|s| !co.contains(s)) {
            let line = line_of_comment(new, s);
            sink.emit_simple(ctx, EventKind::Comment, Direction::Added, "comment", (line, line + 1), None);
        }
        for s in co.iter().filter(|s| !cn.contains(s)) {
            let line = line_of_comment(old, s);
            sink.emit_simple(ctx, EventKind::Comment, Direction::Removed, "comment", (line, line + 1), None);
        }
    }

    // 3) expression：已配对定义内的 Loose 语句
    let old_defs = crate::ast::definitions(old);
    let new_defs = crate::ast::definitions(new);
    let pairs = crate::ast::pair::pair_definitions(
        &old_defs.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
        &new_defs.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
    );
    let mut emitted_ranges: Vec<(u32, u32)> = Vec::new();
    for p in pairs {
        let crate::ast::pair::Pair::Matched { old: oi, new: ni } = p else { continue };
        let osl = super::scope_diff::statement_list(old, old_defs[oi].1);
        let nsl = super::scope_diff::statement_list(new, new_defs[ni].1);
        let scope = old_defs[oi].0.clone();
        for pair in super::scope_diff::align_statements(&osl, &nsl) {
            // Loose 的定义就是"骨架相同、文本不同"，align 已保证；
            // 骨架不同的情况在 align 里是 Removed+Added，不会走到这里。
            // 因此**不需要再查一次 skeleton**（Task 10 的语句检测器已覆盖那种情况）。
            let super::scope_diff::StmtPair::Loose { new: b, .. } = pair else { continue };
            let s = &nsl[b];
            if !emitted_ranges.contains(&s.range) {
                emitted_ranges.push(s.range);
                sink.emit_simple(ctx, EventKind::Expression, Direction::Modified, &s.kind, s.range, Some(scope.clone()));
            }
        }
    }
    emitted_ranges.sort_unstable();

    // 4) test_case：路径判定 + 用例形态
    if super::detect::is_test_path(ctx.file.diff.newest_path()) {
        for n in test_case_nodes(new) {
            if !test_case_nodes(old).iter().any(|o| o.name == n.name) {
                sink.emit_simple(ctx, EventKind::TestCase, Direction::Added, n.node_kind, (n.line, n.line + 1), None);
            }
        }
        for o in test_case_nodes(old) {
            if !test_case_nodes(new).iter().any(|n| n.name == o.name) {
                sink.emit_simple(ctx, EventKind::TestCase, Direction::Removed, o.node_kind, (o.line, o.line + 1), None);
            }
        }
    }
}

struct TestCaseNode { name: String, node_kind: String, line: u32 }

/// 三种用例形态（以 Task 6 的 kinds.tsv 为准）：
///  (1) JS/TS 的 `it("x", ...)` / `test("x", ...)` call，取第一个字符串参数当名字
///  (2) JS/TS 的 `function test_x() {}`
///  (3) Python 的 `def test_x(): ...`
fn test_case_nodes(pf: &ParsedFile) -> Vec<TestCaseNode> {
    let mut out = Vec::new();
    let mut stack = vec![pf.root()];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "function_declaration" | "function_definition" => {
                if let Some(name) = crate::ast::definition_name(n, &pf.source) {
                    if name.starts_with("test") || name.starts_with("Test") {
                        out.push(TestCaseNode {
                            name, node_kind: n.kind().to_string(),
                            line: n.start_position().row as u32 + 1,
                        });
                    }
                }
            }
            // call_expression（JS）/ call（Python）
            "call_expression" | "call" => {
                let fn_name = n.child_by_field_name("function")
                    .or_else(|| n.named_child(0))
                    .map(|f| pf.text_of(f).to_string());
                if matches!(fn_name.as_deref(), Some("it") | Some("test") | Some("describe")) {
                    if let Some(arg) = n.named_children(&mut n.walk()).find(|a| a.kind() == "string") {
                        out.push(TestCaseNode {
                            name: pf.text_of(arg).to_string(),
                            node_kind: n.kind().to_string(),
                            line: n.start_position().row as u32 + 1,
                        });
                    }
                }
            }
            _ => {}
        }
        let mut c = n.walk();
        for ch in n.children(&mut c) { stack.push(ch); }
    }
    out.sort_by(|a, b| (&a.name, a.line).cmp(&(&b.name, b.line)));
    out
}

/// 定位某条注释文本所在行。返回 0 只可能出现在两侧注释集合算错的情形，
/// 用 debug_assert 拦住，不让它悄悄产出一条 range=(0,1) 的事件。
fn line_of_comment(pf: &ParsedFile, text: &str) -> u32 {
    let mut stack = vec![pf.root()];
    while let Some(n) = stack.pop() {
        if n.kind() == "comment" && pf.text_of(n).trim() == text {
            return n.start_position().row as u32 + 1;
        }
        let mut c = n.walk();
        for ch in n.children(&mut c) { stack.push(ch); }
    }
    debug_assert!(false, "注释文本不在树里：{text}");
    0
}
```

`expression` 与 `guard`/`control_flow`/`error_handling` 的互斥不靠"再查一次骨架"，而靠 `align_statements` 的分类本身：骨架不同 → `Removed + Added`（只有语句检测器看它），骨架相同文本不同 → `Loose`（只有叶子检测器看它）。所以 `has_statement_skeleton_change` 这个函数**不存在**，Task 10 也不需要 `Loose` 分支。这个互斥由 `tests/detectors_leaves.rs::formatting_and_expression_are_mutually_exclusive` 与 `tests/detectors_statements.rs::wrapping_in_try_does_not_churn_inner_statements` 两面夹住。

- [ ] **Step 3: `definition_name` 复用与 kinds.tsv 复核**

`test_case_nodes` 用了 `crate::ast::definition_name`（Task 6 Step 3 已定义）。跑之前先复核：Step 2 里出现的 `call_expression` / `call` / `string` 三个 kind 必须在 `tests/fixtures/typescript.kinds.tsv` 与 `python.kinds.tsv` 里存在，否则相应分支是死代码，`test_case_added_in_test_file` / `python_test_function_added` 会失败——那是**提醒你去补 kind**，不是去放宽测试。同时把这两个 kind 补进 `NON_EVENT_KINDS` 或新增一张 `TEST_BODY_KINDS`，让 `every_kind_is_classified` 继续通过。

- [ ] **Step 4: DETECTORS 收尾并跑全量测试**

`src/events/detect.rs` 的 `DETECTORS` 最终为 6 项，顺序：`file_level`、`dependency`、`config_key`、`definition_level`、`statement_level`、`leaf_level`。**同步更新 `tests/detectors_file.rs::detector_order_is_frozen`**，并在断言上方注释写明：顺序改变 = `--dump-events` 输出改变 = 金标准全部重跑。

Run: `cargo test --locked` → 全 PASS。

- [ ] **Step 5: 提交**

```bash
git add src/events tests/detectors_leaves.rs tests/golden tests/golden_worktree src/events/detect.rs
git commit -m "feat: 叶子事件与折叠优先（expression/comment/formatting/test_case）

折叠必须早于分类：否则同一处变更同时是 formatting 和 expression，
W2 的先验表会收到自相矛盾的输入。
骨架不同（kind/子节点数）交给 statement 检测器，不重复报 expression。
test_case 只在测试路径判定成立时产生，避免把集成脚本当测试。
DETECTORS 6 项顺序冻结并有断言守卫。"
```

---

### Task 12: hunk_only 路径

覆盖 spec §6 第二档。**旗舰 UX（`git diff | diffintent` 不带 repo）走的就是这一档**，所以它不是边缘路径；同时 spec R2 指出首屏 demo 很可能落在这里，因此 tier 必须显式可见。

**Files:**
- Create: `src/events/fragment.rs`
- Modify: `src/events/mod.rs`（`extract_events` 分流）
- Test: `tests/fragment.rs`

**Interfaces:**
- Consumes: Task 6 `parse_fragment`；Task 8–11 检测器（部分）
- Produces:
  - `events::fragment::hunk_pair(file: &FileDiff, h: &Hunk, lang: Language) -> Option<(ParsedFile, ParsedFile)>`
  - `events::Debg`→ 统一命名 `ExtractOutcome { events: Vec<Event>, degradations: Vec<Degradation> }`，`Degradation { file: FileId, reason: TierReason, note: String }`
  - `NoAstMatch` 计数（`unknown[].reason_code = "no_ast_match"` 的前身，W3 用）

- [ ] **Step 1: 写失败测试**

`tests/fragment.rs`：

```rust
use diffintent::seir::{Direction, EvidenceTier, EventKind};
use diffintent_test_support::*;

/// 纯管道输入（无工作树）必须产 hunk_only 事件，且 tier 标出来。
#[test]
fn no_worktree_yields_hunk_only_events() {
    let diff = std::fs::read_to_string("tests/golden/mixed.diff").unwrap();
    let out = run_pipeline(&diff, None);
    assert!(out.events.iter().all(|e| e.tier == EvidenceTier::HunkOnly),
        "{:?}", out.events.iter().map(|e| (e.kind, e.tier)).collect::<Vec<_>>());
}

/// 片段必然不完整，所以不得假装拿到完整定义结构：
/// guard/control_flow/error_handling 一律不产（与 Task 10 的 hunk_only 测试同源）。
#[test]
fn fragment_tier_emits_no_statement_level_events() {
    let diff = std::fs::read_to_string("tests/golden/multi_hunk.diff").unwrap();
    let out = run_pipeline(&diff, None);
    for e in &out.events {
        assert!(!matches!(e.kind, EventKind::Guard | EventKind::ControlFlow | EventKind::ErrorHandling),
            "hunk_only 不得产出 {} 级信号: {:?}", e.kind, e);
    }
}

/// 无法解析的片段必须记 no_ast_match，而不是静默零事件。
/// 静默零事件 = "这个 diff 什么都没改"，是最坏的一种谎。
#[test]
fn unparseable_fragment_records_no_ast_match() {
    let diff = "diff --git a/src/a.ts b/src/a.ts\nindex 0..1 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-const a = {{{\n+const a = }}}\n";
    let out = run_pipeline(diff, None);
    assert!(out.degradations.iter().any(|d| d.note.contains("no_ast_match")), "{:?}", out.degradations);
}

/// scope 在 hunk_only 下通常是 None，但**片段里恰好含完整定义时可以有**。
/// 不许一刀切成 None —— 那会把本可判定的 hunk 也拖进 unknown。
#[test]
fn fragment_can_still_resolve_scope_when_definition_is_fully_inside() {
    let diff = single_hunk_full_function(
        "function a() {\n  return 1;\n}",
        "function a() {\n  return 2;\n}",
    );
    let out = run_pipeline(&diff, None);
    assert!(out.events.iter().any(|e| e.scope.as_ref().map(|s| s.name == "a").unwrap_or(false)),
        "{:?}", out.events);
}

/// 片段必须补 enough 上下文才可能成 statement：从 hunk 的 Context 行还原，
/// 而不是只喂 + 行。只喂 + 行会让 `if (!x) {` 变成孤立左括号。
#[test]
fn fragment_includes_context_lines() {
    let diff = std::fs::read_to_string("tests/golden/multi_hunk.diff").unwrap();
    let out = run_pipeline(&diff, None);
    assert!(!out.events.is_empty(), "事件为空说明片段没带上下文，全是孤立行");
    assert!(out.degradations.is_empty() || out.events.len() > 0);
}

/// 同一个 diff：有工作树 vs 无工作树，**tier 必须不同且都非空**。
/// 这条挡住"两种输入产出一样结果"的假实现（那意味着工作树根本没被用）。
#[test]
fn same_diff_two_tiers() {
    let (dir, diff) = common::real_repo(
        &[("src/a.ts", "export function a() {\n  return 1;\n}\n")],
        &[("src/a.ts", "export function a() {\n  if (!v) return 0;\n  return 1;\n}\n")],
    );
    let wt = Worktree::new(Some(dir)).unwrap();
    let full = run_pipeline(&diff, Some(&wt));
    let frag = run_pipeline(&diff, None);
    assert!(full.events.iter().all(|e| e.tier == EvidenceTier::FullAst));
    assert!(frag.events.iter().all(|e| e.tier == EvidenceTier::HunkOnly));
    assert!(full.events.iter().any(|e| e.kind == EventKind::Guard), "{:?}", full.events);
    assert!(!frag.events.iter().any(|e| e.kind == EventKind::Guard), "{:?}", frag.events);
}
```

Run: `cargo test --locked --test fragment` → Expected: 编译失败。

- [ ] **Step 2: 片段构造**

`src/events/fragment.rs`：

```rust
//! spec §6 第二档。片段解析必然不完整，故：
//! 1) 允许 ERROR 节点（parse_fragment 的宽松性）
//! 2) 但**只信 statement 边界清晰的事件**；定义级/语句级信号一律不出
//! 3) 解析失败必须记 no_ast_match，不得静默返回空

use crate::ast::ParsedFile;
use crate::evidence::Language;
use crate::patch::{DiffLine, FileDiff, Hunk};

/// 把一个 hunk 还原成"前镜像片段"与"后镜像片段"。
/// 关键：**两侧都带 Context 行**。只喂 +/- 行会让 `if (!x) {` 变成孤立左括号，
/// 片段解析全成 ERROR，于是整个 hunk_only 档退化成 no_ast_match。
pub fn hunk_pair(file: &FileDiff, h: &Hunk, lang: Language) -> Option<(ParsedFile, ParsedFile)> {
    let mut pre = String::new();
    let mut post = String::new();
    for l in &h.lines {
        match l {
            DiffLine::Context(s) => { pre.push_str(s); pre.push('\n'); post.push_str(s); post.push('\n'); }
            DiffLine::Removed(s) => { pre.push_str(s); pre.push('\n'); }
            DiffLine::Added(s) => { post.push_str(s); post.push('\n'); }
            DiffLine::NoNewline => {}
        }
    }
    // 补齐外层闭合括号：函数体被截断时补 `}`，否则必成 ERROR。
    let pre_s = balance(&pre);
    let post_s = balance(&post);
    let old = ParsedFile::parse_fragment(lang, &pre_s)?;
    let new = ParsedFile::parse_fragment(lang, &post_s)?;
    Some((old, new))
}

/// 补足未闭合的 `{`/`}`。只做数量平衡，不做语法分析——
/// 目标是让 tree-sitter 少产 ERROR，不是造出合法代码。
fn balance(src: &str) -> String {
    let opens = src.matches('{').count();
    let closes = src.matches('}').count();
    let mut out = src.to_string();
    if opens > closes {
        for _ in 0..(opens - closes) { out.push_str("\n}\n"); }
    } else if closes > opens {
        let mut prefix = String::new();
        for _ in 0..(closes - opens) { prefix.push_str("{\n"); }
        // 前缀补 `{` 会让行号偏移，故只做记录、不改内容
        let _ = prefix;
    }
    out
}

/// 片段是否可信到足以出 scope。
pub fn trust_scope(new_frag: &ParsedFile) -> bool {
    !new_frag.has_error_nodes() && crate::ast::definitions(new_frag).len() == 1
}
```

`balance` 的 `closes > opens` 分支现在什么也不做——**保留它但注释写明为什么不做**（前缀补 `{` 会让 `range` 行号与 diff 声明的行号错位，而 `Event.range` 是契约字段，宁可留 ERROR 也不制造错位）。这个决定本身进测试：`unparseable_fragment_records_no_ast_match` 就是它的守卫。

- [ ] **Step 3: extract_events 分流 + 降级记录**

`src/events/mod.rs`：

```rust
pub struct Degradation {
    pub file: FileId,
    pub reason: TierReason,
    pub note: String,
}

pub struct ExtractOutcome {
    pub events: Vec<Event>,
    pub degradations: Vec<Degradation>,
}

pub fn extract_events(files: &[ResolvedFile]) -> ExtractOutcome {
    let mut all = Vec::new();
    let mut degs = Vec::new();
    for f in files {
        match f.tier {
            EvidenceTier::FullAst => { /* 现有完整文件路径 */ }
            EvidenceTier::HunkOnly => {
                let lang = match f.language { Some(l) => l, None => continue };
                let mut any = false;
                for h in &f.diff.hunks {
                    match fragment::hunk_pair(&f.diff, h, lang) {
                        Some((_, new)) => {
                            let ctx = DetectorCtx { file: f, old: None, new: Some(&new) };
                            let mut sink = EventSink::default();
                            // 只跑文件级与依赖/配置：语句级需要完整函数体
                            for d in detect::DETECTORS {
                                if matches!(d.name, "file_level" | "dependency" | "config_key") {
                                    (d.run)(&ctx, &mut sink);
                                }
                            }
                            let mut ev = sink.into_events();
                            for e in &mut ev { e.tier = EvidenceTier::HunkOnly; }
                            if !ev.is_empty() { any = true; }
                            all.extend(ev);
                        }
                        None => degs.push(Degradation {
                            file: f.id,
                            reason: TierReason::ParseFailed,
                            note: format!("no_ast_match: fragment at new_start {}", h.new_start),
                        }),
                    }
                }
                if !any && !f.diff.hunks.is_empty() {
                    degs.push(Degradation { file: f.id, reason: TierReason::ParseFailed, note: "no_ast_match: all fragments unresolvable".into() });
                }
            }
            EvidenceTier::TextFallback => { /* Task 13 */ }
        }
    }
    canonical_sort(&mut all);
    ExtractOutcome { events: all, degradations: degs }
}
```

`f.diff.hunks` 为空时不得推入 degradation（纯 rename / mode-only 本就无事发生）。

- [ ] **Step 4: 跑测试**

`cargo test --locked` → 全 PASS。`same_diff_two_tiers` 是本任务的核心守卫：它证明工作树真的被用了。

- [ ] **Step 5: 提交**

```bash
git add src/events tests/fragment.rs
git commit -m "feat: hunk_only 片段路径与降级记录

片段两侧都带 Context 行；只喂 +/- 行会让 if (!x) { 变成孤立左括号，
整档退化成 no_ast_match。
balance() 不对 closes > opens 补前缀：那会让 Event.range 与 diff 声明行号错位，
宁可留 ERROR 也不制造契约字段错位。
解析失败必须记 no_ast_match 而非静默返回空——静默空等于宣称"什么都没改"。
same_diff_two_tiers 守卫工作树真的被使用。"
```

---

### Task 13: text_fallback 路径

覆盖 spec §6 第三档 + §6 的"宁可诚实降级，不要假装支持"。本任务的产品要求是**限制它的能力上限并写进测试**，因为这条路径最容易在无人注意时悄悄长出高置信度的猜测。

**Files:**
- Create: `src/events/text_fallback.rs`
- Modify: `src/events/mod.rs`
- Test: `tests/text_fallback.rs`

**Interfaces:**
- Consumes: Task 5 的 `is_config_path` `is_derived_path`
- Produces:
  - `events::text_fallback::detect(file: &ResolvedFile) -> Vec<Event>`
  - `events::text_fallback::looks_generated(path: &str, hunks: &[Hunk]) -> bool`
  - 能力白名单：`file` `module` `dependency` `config_key` `formatting` `comment`

- [ ] **Step 1: 写失败测试**

`tests/text_fallback.rs`：

```rust
use diffintent::seir::{Direction, EvidenceTier, EventKind};
use diffintent_test_support::*;

const ALLOWED: [EventKind; 6] = [
    EventKind::File, EventKind::Module, EventKind::Dependency,
    EventKind::ConfigKey, EventKind::Formatting, EventKind::Comment,
];

/// 不支持的语言**只**允许产出白名单内的 kind。这是 §6 "不假装支持"的代码化。
#[test]
fn unsupported_languages_are_limited_to_a_whitelist() {
    for (path, old, new) in [
        ("src/main.go", "package main\n\nfunc a() {\n\treturn 1\n}\n", "package main\n\nfunc a() {\n\tif x == nil {\n\t\treturn 2\n\t}\n\treturn 1\n}\n"),
        ("src/App.rs", "fn a() -> i32 { 1 }\n", "fn a() -> i32 { if x { return 2 } 1 }\n"),
        ("src/Main.java", "class A { int a() { return 1; } }\n", "class A { int a() { try { return 1; } catch (E e) { return 2; } } }\n"),
        ("src/a.rb", "def a\n  1\nend\n", "def a\n  return nil unless x\n  1\nend\n"),
    ] {
        let out = run_text_fallback(path, old, new);
        for e in &out.events {
            assert!(ALLOWED.contains(&e.kind),
                "{path} 产出了白名单外的 {} —— text_fallback 不得猜结构: {e:?}", e.kind);
            assert_eq!(e.tier, EvidenceTier::TextFallback);
        }
        // 且必须留下一条降级说明
        assert!(out.degradations.iter().any(|d| d.note.contains("grammar_unsupported")), "{path}: {:?}", out.degradations);
    }
}

/// 新增文件在 text_fallback 下仍可判定（存在性不依赖 AST）。
#[test]
fn added_file_is_detectable_without_ast() {
    let out = run_text_fallback_added("src/main.go");
    assert!(out.events.iter().any(|e| e.kind == EventKind::File && e.direction == Direction::Added), "{:?}", out.events);
}

/// 配置文件即使语言不支持也必须出 config_key（spec §5.1：扩展名判定，不需 AST）。
#[test]
fn config_events_survive_text_fallback() {
    let out = run_text_fallback("deploy/k8s.yaml", "a: 1\nb: 2\n", "a: 1\nb: 3\n");
    assert!(out.events.iter().any(|e| e.kind == EventKind::ConfigKey), "{:?}", out.events);
}

/// 生成物识别：.d.ts / *_pb2.py / 单行超长 min.js 不得当普通源文件分析。
#[test]
fn generated_files_are_recognized() {
    assert!(diffintent::events::text_fallback::looks_generated("src/types.d.ts", &[]));
    assert!(diffintent::events::text_fallback::looks_generated("dist/app.min.js", &hunks_with_one_line_of(9000)));
    assert!(!diffintent::events::text_fallback::looks_generated("src/app.ts", &[normal_hunk()]));
}

/// **绝不出具看似 confident 的结构事件**：text_fallback 不得产 signature/control_flow。
/// 与第一条重复但方向不同：第一条管"实际产出的东西"，这条管"最诱人的错误实现"。
#[test]
fn never_emits_shape_dependent_kinds() {
    for path in ["src/a.go", "src/A.java", "src/a.rb", "src/a.php", "src/a.swift", "src/a.c", "src/a.cpp", "src/a.cs", "src/a.kt"] {
        let out = run_text_fallback(path, "old body\nmore\n", "new body\nmore\neven\n");
        for e in &out.events {
            assert!(!matches!(e.kind,
                EventKind::Signature | EventKind::ControlFlow | EventKind::Guard
                | EventKind::ErrorHandling | EventKind::Expression | EventKind::Binding
                | EventKind::Call | EventKind::TestCase),
                "{path}: text_fallback 产出了形状依赖的 {:?}", e.kind);
        }
    }
}

/// `--json` 的 evidence_tiers 必须能把这一档数出来（W3 依赖，但计数正确性在此保证）。
#[test]
fn tier_counts_sum_to_files_changed() {
    let diff = std::fs::read_to_string("tests/golden/crlf.diff").unwrap();
    let out = run_pipeline(&diff, None);
    let mut counts = [0usize; 3];
    for r in &out.resolved { counts[r.tier_index()] += 1; }
    assert_eq!(counts.into_iter().sum::<usize>(), out.resolved.len());
}
```

Run: `cargo test --locked --test text_fallback` → Expected: 编译失败。

- [ ] **Step 2: 降级检测器**

`src/events/text_fallback.rs`：

```rust
//! spec §6 第三档。**能力上限就是本文件的白名单**，写在类型里而非注释里。
//! 加一个 kind 到 ALLOWED_KINDS 之前必须先改 spec §6 —— 这条由
//! tests/text_fallback.rs::unsupported_languages_are_limited_to_a_whitelist 守卫。

use crate::evidence::{is_config_path, ResolvedFile};
use crate::patch::{DiffLine, Hunk};
use crate::seir::{Direction, Event, EventKind, EvidenceTier, FileId};

pub fn detect(file: &ResolvedFile) -> Vec<Event> {
    let mut out = Vec::new();
    let path = file.diff.newest_path();
    let mk = |kind: EventKind, direction: Direction, node: &str, range: (u32, u32)| Event {
        file: file.id, kind, direction, scope: None, node_kind: node.into(), range,
        tier: EvidenceTier::TextFallback,
    };

    if matches!(file.diff.change, crate::patch::FileChange::Added) {
        // 新文件：File 事件确定；Module 只在"看起来有定义"时给，因为 HP2 依赖它（spec §9.3）
        out.push(mk(EventKind::File, Direction::Added, "file", (1, 1)));
        if looks_like_has_definitions(&file.diff) {
            out.push(mk(EventKind::Module, Direction::Added, "file", (1, last_new_line(&file.diff))));
        }
    }
    if matches!(file.diff.change, crate::patch::FileChange::Deleted) {
        out.push(mk(EventKind::File, Direction::Removed, "file", (1, last_old_line(&file.diff))));
    }
    if is_config_path(path) {
        for h in &file.diff.hunks {
            if touched(h) {
                out.push(mk(EventKind::ConfigKey, Direction::Modified, "config", (h.new_start, h.new_start + h.new_count as u32)));
            }
        }
    }
    for h in &file.diff.hunks {
        for l in &h.lines {
            match l {
                DiffLine::Added(s) if is_importish(s, path) => out.push(mk(EventKind::Dependency, Direction::Added, "text_import", (h.new_start, h.new_start))),
                DiffLine::Removed(s) if is_importish(s, path) => out.push(mk(EventKind::Dependency, Direction::Removed, "text_import", (h.old_start, h.old_start))),
                DiffLine::Added(s) if is_commentish(s, path) => out.push(mk(EventKind::Comment, Direction::Added, "text_comment", (h.new_start, h.new_start))),
                DiffLine::Removed(s) if is_commentish(s, path) => out.push(mk(EventKind::Comment, Direction::Removed, "text_comment", (h.old_start, h.old_start))),
                _ => {}
            }
        }
    }
    out
}

fn is_importish(line: &str, path: &str) -> bool {
    let t = line.trim_start();
    match path.rsplit('.').next().unwrap_or("") {
        "go" => t.starts_with("import ") || t.starts_with("\""),
        "java" | "kt" => t.starts_with("import "),
        "cs" => t.starts_with("using ") && t.ends_with(';'),
        "rb" => t.starts_with("require") || t.starts_with("load "),
        "php" => t.starts_with("use ") || t.starts_with("require"),
        "c" | "cpp" | "h" | "hpp" => t.starts_with("#include"),
        "rs" => t.starts_with("use ") || t.starts_with("extern crate"),
        _ => false,
    }
}

fn is_commentish(line: &str, path: &str) -> bool {
    let t = line.trim_start();
    match path.rsplit('.').next().unwrap_or("") {
        "py" | "rb" | "sh" | "yaml" | "yml" | "toml" => t.starts_with('#'),
        "go" | "java" | "kt" | "cs" | "php" | "js" | "ts" | "c" | "cpp" | "rs" => t.starts_with("//"),
        _ => false,
    }
}

/// 行级"有定义"启发：出现函数/类关键字且左括号在行尾。
/// 只用于决定要不要给 module:added（HP2 的输入），**不用于任何意图分类**。
fn looks_like_has_definitions(file: &crate::patch::FileDiff) -> bool {
    file.diff_hunks_added_lines().iter().any(|l| {
        let t = l.trim();
        t.contains("function ") || t.contains("func ") || t.contains("def ")
            || t.contains("class ") || t.contains("interface ") || t.contains("struct ")
            || t.contains("impl ") || t.contains("type ")
    })
}

/// spec §9.4 的补充：生成物识别（除 is_derived_path 之外的形态线索）。
pub fn looks_generated(path: &str, hunks: &[Hunk]) -> bool {
    let p = path.to_ascii_lowercase();
    if p.ends_with(".d.ts") || p.ends_with("_pb2.py") || p.ends_with(".min.js") || p.ends_with(".min.css") {
        return true;
    }
    // 单行超长（bundle/压缩产物）或单 hunk 行数异常大
    hunks.iter().any(|h| h.lines.iter().any(|l| line_text(l).len() > 2000))
        || hunks.iter().any(|h| h.new_count > 5000)
}

fn line_text(l: &DiffLine) -> &str {
    match l { DiffLine::Context(s) | DiffLine::Added(s) | DiffLine::Removed(s) => s, DiffLine::NoNewline => "" }
}
```

`file.diff_hunks_added_lines()` 需在 `FileDiff` 上加一个小 helper（返回所有 `Added` 行的 `&[&str]`）。`looks_generated` 在 `detect` 里未被使用——**在 `extract_events` 的 TextFallback 分支调用它**，命中时只出 `File` 事件、不出 `Module`/`ConfigKey`，并在 `Degradation.note` 写 `generated: <reason>`（否则 `dist/bundle.js` 的单行超长 diff 会被当成"新增了一个模块"从而触发 HP2 误报，正是 spec §9.4 要挡的）。

- [ ] **Step 3: 跑测试并接线**

`cargo test --locked` → 全 PASS。`never_emits_shape_dependent_kinds` 若挂，说明 `detect` 里长出了不该有的形状判断——删掉它，不要扩展白名单。

- [ ] **Step 4: 提交**

```bash
git add src/events/text_fallback.rs src/events/mod.rs tests/text_fallback.rs src/patch/model.rs
git commit -m "feat: text_fallback 降级路径与能力白名单

白名单写进测试（unsupported_languages_are_limited_to_a_whitelist），
加 kind 必须先改 spec §6。这是"不假装支持"的代码化而不是文档承诺。
looks_like_has_definitions 只用于决定 module:added（HP2 输入），不用于任何意图分类。
生成物（.d.ts/.min.js/_pb2.py/单行超长）单独判定，否则 bundle diff 会被当成
新增模块触发 HP2 误报 —— spec §9.4 正是要挡这个。"
```

---

### Task 14: CLI 接线、端到端确定性与预算复核

把 `--dump-events` 打通到真实输入，并让 spec §4.1 的"同 diff 同输出"在 CLI 层面成立（Task 2 只在类型层面证过）。同时把性能预算解除 `#[ignore]`，并给 R2（README demo 落在低 tier）提供可见性。

**Files:**
- Modify: `src/main.rs`, `src/lib.rs`
- Create: `src/render.rs`
- Modify: `tests/budget.rs`
- Test: `tests/determinism.rs`

**Interfaces:**
- Consumes: 全部
- Produces:
  - `lib::analyze(text: &str, worktree: Option<&Worktree>, request: Option<&str>) -> Result<Analysis, AnalyzeError>`
  - `Analysis { resolved: Vec<ResolvedFile>, events: Vec<Event>, degradations: Vec<Degradation>, request: Option<String>, unsupported: usize }`
  - `render::dump_events(&Analysis) -> String`（serde_pretty，规范序）
  - `render::tiers_banner(&Analysis) -> String`（终端首行 `full_ast 2/3 files` —— R2 要求 tier 不得被隐藏）
  - 退出码 0/3 在此接线；1/2 留 W3

- [ ] **Step 1: 写端到端确定性测试**

`tests/determinism.rs`：

```rust
use diffintent::{analyze, Worktree};

fn read_diff(name: &str) -> String { std::fs::read_to_string(format!("tests/golden/{name}")).unwrap() }

/// 逐字节相同，跑两次。
#[test]
fn same_input_same_bytes() {
    let d = read_diff("mixed.diff");
    let a = render_of(&d);
    let b = render_of(&d);
    assert_eq!(a, b);
}

/// 打乱 diff 里文件块的顺序 → 逐字节相同。
/// 这是 spec §4.1 的最终守卫：FileTable 按路径字典序赋值（Task 2）在这一条上被验证。
#[test]
fn shuffled_file_order_yields_identical_bytes() {
    let d = read_diff("mixed.diff");
    let orig = render_of(&d);
    let shuffled = shuffle_file_blocks(&d);
    let got = render_of(&shuffled);
    assert_eq!(orig, got, "文件顺序影响输出：说明某处仍依赖遇到顺序");
}

/// 反向顺序再来一次（两次不同排列都不得改变结果）。
#[test]
fn reversed_blocks_yields_identical_bytes() {
    let d = read_diff("mixed.diff");
    let orig = render_of(&d);
    let mut blocks = split_blocks(&d);
    blocks.reverse();
    assert_eq!(orig, render_of(&blocks.join("")), "{:?}", blocks.iter().map(|b| b.lines().next()).collect::<Vec<_>>());
}

fn render_of(text: &str) -> String {
    let a = analyze(text, None, None).expect("analyze");
    diffintent::render::dump_events(&a)
}

/// 按 `diff --git ` 前缀切块。开头的非 diff 前言并入第一块。
/// 必须用 `split_inclusive('\n')` 保留行尾换行：`lines()` 会丢换行，
/// 重组后的文本与原文不再是同一份输入，那会让本测试测不到它想测的东西。
fn split_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("diff --git ") && !cur.is_empty() {
            blocks.push(std::mem::take(&mut cur));
        }
        cur.push_str(line);
    }
    if !cur.is_empty() { blocks.push(cur); }
    blocks
}

fn shuffle_file_blocks(text: &str) -> String {
    let mut b = split_blocks(text);
    if b.len() > 1 { b.swap(0, b.len() - 1); }
    b.join("")
}

/// tier 计数必须等于文件数（R2：tier 不得被隐藏，banner 是第一行第一眼）。
#[test]
fn tiers_banner_shows_every_file() {
    let a = analyze(&read_diff("mixed.diff"), None, None).unwrap();
    let banner = diffintent::render::tiers_banner(&a);
    assert!(banner.contains("files"), "{banner}");
    let sum: usize = a.resolved.iter().map(|r| 1usize).sum();
    assert_eq!(sum, 6, "夹具是 6 个文件");
    assert!(banner.contains(&format!("{sum}")), "{banner}");
}

/// stdin 空输入 → 退出码 3 而非 panic，且 stderr 有可操作的说明。
#[test]
fn empty_input_exits_3_with_message() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_diffintent"))
        .stdin(std::process::Stdio::piped())
        .output()
        .unwrap();
    // 空 stdin 视为无 diff：这不是错误，应当成功且零事件
    assert!(out.status.success() || out.status.code() == Some(3));
}

/// 非 diff 输入必须报 3。
#[test]
fn non_diff_input_exits_3() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_diffintent"))
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| { c.stdin.take().unwrap().write_all(b"On branch main\nnothing to commit\n").map(|_| c) })
        .unwrap()
        .wait_with_output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

/// 冷启动：--version 必须 < 80 ms（真实预算，非 CI 的 160）。
#[test]
fn version_exits_fast() {
    let t = std::time::Instant::now();
    for _ in 0..5 {
        let _ = std::process::Command::new(env!("CARGO_BIN_EXE_diffintent")).arg("--version").output().unwrap();
    }
    let ms = t.elapsed().as_millis() / 5;
    println!("--version avg {ms} ms");
    assert!(ms < 80, "冷启动 {ms} ms 超 spec §12 预算");
}
```

`write_all` 需 `use std::io::Write;`。`split_blocks` 在 Step 4 实现（按 `diff --git ` 前缀切，首块若不是该行开头则并入首块，`index`/`old mode` 等扩展行属于块内）。

- [ ] **Step 2: lib 入口**

`src/lib.rs` 追加：

```rust
pub mod events;
pub mod render;

use contract::TierReason;
use evidence::{resolve, Worktree};
use patch::PatchError;
use seir::{Event, FileId};

#[derive(Debug, thiserror::Error)]
pub enum AnalyzeError {
    #[error(transparent)]
    Patch(#[from] PatchError),
}

pub struct Degradation {
    pub file: FileId,
    pub reason: TierReason,
    pub note: String,
}

pub struct Analysis {
    pub resolved: Vec<evidence::ResolvedFile>,
    pub events: Vec<Event>,
    pub degradations: Vec<Degradation>,
    pub request: Option<String>,
    /// 走 text_fallback 的文件数（W2/W3 的 unsupported 统计入口）
    pub unsupported: usize,
}

pub fn analyze(text: &str, worktree: Option<&Worktree>, request: Option<&str>) -> Result<Analysis, AnalyzeError> {
    let files = patch::parse(text)?;
    let resolved = resolve(files, worktree);
    let out = events::extract_events(&resolved);
    let unsupported = resolved.iter().filter(|r| r.tier == seir::EvidenceTier::TextFallback).count();
    Ok(Analysis { resolved, events: out.events, degradations: out.degradations, request: request.map(str::to_string), unsupported })
}
```

`Degradation` 从 `events` 移到 `lib`（避免两个定义），`events::mod` 里 `pub use crate::Degradation;`。

- [ ] **Step 3: render**

`src/render.rs`：

```rust
//! W1 的渲染只有两个面：--dump-events 与 tier banner。
//! **不存在第二套事件渲染**（spec §3.1：同一棵内部树）。

use crate::Analysis;

pub fn dump_events(a: &Analysis) -> String {
    serde_json::to_string_pretty(&a.events).unwrap_or_default() + "\n"
}

/// spec R2：tier 不得被隐藏。这一行进 --json 与终端输出两处。
pub fn tiers_banner(a: &Analysis) -> String {
    let mut full = 0; let mut hunk = 0; let mut text = 0;
    for r in &a.resolved {
        match r.tier {
            crate::seir::EvidenceTier::FullAst => full += 1,
            crate::seir::EvidenceTier::HunkOnly => hunk += 1,
            crate::seir::EvidenceTier::TextFallback => text += 1,
        }
    }
    let total = a.resolved.len();
    let mut s = format!("evidence: full_ast {full}/{total}");
    if hunk > 0 { s.push_str(&format!(", hunk_only {hunk}/{total}")); }
    if text > 0 { s.push_str(&format!(", text_fallback {text}/{total}")); }
    s
}
```

- [ ] **Step 4: main 接线 + 退出码**

`src/main.rs`：

```rust
use clap::Parser;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "diffintent", version, about = "intent-level diff analysis, zero LLM by construction")]
struct Cli { /* 字段同 Task 1 Step 4 */ }

fn main() {
    let cli = Cli::parse();
    let code = run(cli);
    std::process::exit(code);
}

fn run(cli: Cli) -> i32 {
    let text = match read_input(&cli.file) {
        Ok(t) => t,
        Err(e) => { eprintln!("error: {e}"); return 3; }
    };
    if text.trim().is_empty() {
        println!("no diff input");
        return 0;
    }
    let wt_root = cli.repo.clone().or_else(|| Some(PathBuf::from(".")));
    let wt = wt_root.and_then(evidence::Worktree::new);
    let a = match diffintent::analyze(&text, wt.as_ref(), cli.request.as_deref()) {
        Ok(a) => a,
        Err(e) => { eprintln!("error: {e}"); return 3; }
    };
    if cli.dump_events {
        print!("{}", diffintent::render::dump_events(&a));
        println!("{}", diffintent::render::tiers_banner(&a));
    } else {
        println!("{}", diffintent::render::tiers_banner(&a));
        println!("(W1: 意图分类尚未实现。spec §17 里程碑 2 起提供。用 --dump-events 看结构事件)");
    }
    for d in &a.degradations {
        if cli.details { eprintln!("degraded: {} {}", d.file.0, d.note); }
    }
    0
}

fn read_input(file: &Option<PathBuf>) -> Result<String, std::io::Error> {
    match file {
        Some(p) => std::fs::read_to_string(p),
        None => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; Ok(s) }
    }
}
```

`use std::io::Read` 在 `read_input` 用到；`Write` 若未用则删（clippy）。`--fail-on-drift` / `--fail-on-unknown` 在此**接受但忽略**，并在输出里打印一行 `note: --fail-on-drift requires W3 (drift_align)`——静默忽略 flag 会让人以为门禁生效了，这是最坏的失败。

- [ ] **Step 5: 解除性能测试 ignore 并复核预算**

`tests/budget.rs` 里把 `#[ignore = ...]` 改成 `#[ignore = "需 --release；CI 跑"]`，并把调用改为新的 `diffintent::analyze` 签名（无工作树，走 hunk_only，这是最坏情况路径，符合预算测试该测的方向）：

```rust
#[test]
#[ignore = "需 --release；由 scripts/check-budget.sh 与 CI 调用"]
fn fifty_file_two_thousand_line_under_600ms() {
    let text = synthetic_diff(50, 20);
    let start = Instant::now();
    let a = diffintent::analyze(&text, None, None).expect("analyze");
    let ms = start.elapsed().as_millis();
    println!("50f/2k diff (hunk_only path): {ms} ms, {} events", a.events.len());
    assert!(ms < 600, "超预算: {ms} ms");
    assert!(!a.events.is_empty(), "零事件说明片段路径整体失败，性能数字没有意义");
}
```

再补一条 full_ast 路径的对照（性能差 tier 一档是预期，但必须能报出数字给 W2 的校准分桶当依据）：

```rust
#[test]
#[ignore]
fn fifty_file_full_ast_path_reports_its_cost() {
    // 建 50 个真实文件的工作树 + 对应 diff
    let (wt, diff) = common::synthetic_repo(50, 20);
    let start = Instant::now();
    let a = diffintent::analyze(&diff, Some(&wt), None).expect("analyze");
    let ms = start.elapsed().as_millis();
    println!("50f full_ast: {ms} ms, {} events", a.events.len());
    assert!(ms < 600, "full_ast 超预算: {ms} ms");
}
```

跑：`cargo test --release --locked -- --ignored`，把两个数字记进 PR 描述。

- [ ] **Step 6: 体积复核并记录**

```bash
cargo build --release --locked && bash scripts/check-budget.sh target/release/diffintent && bash scripts/check-no-network.sh
```

Expected: 体积与冷启动达标，禁网绿。把实测三个数字（体积 / 冷启动 / 50f 两档耗时）写进 commit body——spec R2 的"README demo 必须落在 full_ast"要据此决定（若 hunk_only 慢到吃不住预算，demo 更必须用带工作树的方式跑）。

- [ ] **Step 7: 全量绿**

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Expected: 三条全过。任何一条不过就地修，**不要放宽 lint**。

- [ ] **Step 8: 提交**

```bash
git add src/main.rs src/lib.rs src/render.rs tests/determinism.rs tests/budget.rs
git commit -m "feat: CLI 接线、端到端确定性与预算复核

打乱 diff 文件块顺序 → 输出逐字节相同：这条在 CLI 层验证 Task 2 的
FileTable 按路径字典序赋值（类型层证过不等于端到端成立）。
tiers_banner 把证据层级放在第一行；spec R2 明令 tier 不得被隐藏。
--fail-on-drift/--fail-on-unknown 在 W1 接受但打印提示而非静默忽略——
静默忽略会让人以为门禁生效了。
实测：体积 X MB / 冷启动 X ms / 50f hunk_only X ms / full_ast X ms。"
```

---

## 计划自查

**1. spec 覆盖。** 本计划实现 spec §4 的第 1–3 段、§5（14 个 kind 全覆盖：Task 8 出 `file`/`dependency`/`module`/`config_key`，Task 9 出 `signature`/`binding`/`call`，Task 10 出 `control_flow`/`guard`/`error_handling`，Task 11 出 `expression`/`formatting`/`comment`/`test_case`）、§6 三档（Task 5/12/13）、§4.1 确定性不变量（Task 2/14）、§10 禁网门禁（Task 1）、§12 的体积/冷启动/性能三项（Task 1/14）、R2（Task 14 banner）、R3（Task 1 Windows 进 CI 矩阵）、R7 的 `request_lang` 前置（Task 14 接受 `--request` 但本计划不使用它）。

**未覆盖且属后续计划**：§4 第 4–7 段（`block_segment` 与 `prior_score` 属 W2；`drift_align` 属 W3；`render` 完整形态属 W3）、§3 完整契约与快照测试（W3）、§8.2 `CALIBRATION`（W4）、§9 HP 触发器与 `derived_path` 的**使用**（W3；`is_derived_path` 的判定已在 Task 5 就位）、§11 评测集（W2）、§13 分发（W4）、SKILL.md（W4）。

**2. 占位符扫描。** 计划里有三类"看起来像占位符"的东西，逐一说明其性质，它们都不允许被执行者当成"回头再补"：
- Task 6 Step 3 的 `definitions` / `enclosing_scope` 写成 `todo!` 并附明确算法，因为 Step 4 的测试先于实现存在——这是 TDD 顺序，不是缺口。执行者必须让 Step 4 的 8 条测试通过才能提交。
- Task 9/10/11 的 `tests/common` 里的 `events_ts` / `events_py` / `events_*_at` / `events_hunk_only` / `ALL_KINDS_14` / `run_case_with_sources` / `run_text_fallback` / `synthetic_repo` 是**跨任务共享的测试脚手架**。落地顺序：Task 4 Step 1 建 `tests/common/mod.rs`（`git_in`/`git_bytes_in`/`write_in`/`real_repo`/`git_diff_head1`），Task 8 加 `run_case_with_sources`，Task 9 加 `events_ts`/`events_py`/`events_*_at`，Task 10 加 `ALL_KINDS_14` 与 `events_hunk_only`，Task 13 加 `run_text_fallback`/`hunks_with_one_line_of`/`normal_hunk`，Task 14 加 `synthetic_repo`。**每个 helper 必须在引入它的那次提交里实现完整**，不留 `unimplemented!()`。
- 计划里凡写"以 kinds.tsv 实测为准""用 `cargo doc -p tree-sitter-typescript` 确认常量名""按编译器报错改"的地方，都是**故意的**：tree-sitter 各版本的 Rust API 形态（`LANGUAGE` 与 `LANGUAGE_TYPESCRIPT` 的命名、`Parser::parse` 返回 `Option<Tree>` 还是 `Result`、`Node` 的生命周期写法）、语法 crate build script 读取的环境变量名、node kind 的确切拼写，都不应在没有 `cargo` 的机器上凭记忆钉死。这些地方计划给的是**判据与测试断言**，不是字面常量；两者冲突时以本机实测为准，并在 commit body 记录实测到的名字。

**3. 类型一致性。** 逐处核对过并就地改掉：`EventSink` 统一为结构体（`push`/`into_events`），不用别名；`Direction` 只有三值（Task 6 草稿里的 `Moved` 与 `KindChange` 已删——moved 检测在 spec §5.2 被明确排除，`async` 变化按 `control_flow` 报，Task 10 Step 1 有测试钉住这条）；`extract_events` 返回 `ExtractOutcome`（Task 10 草稿里的 `Option<Vec<Event>>` 会让降级原因无处安放，导致 `summary.evidence_tiers` 与 block 的 `evidence_tier` 不一致，属 spec R2 要防的问题）；`FileId`/`Event`/`ScopeRef` 字段在 Task 2 定义后未再改动；`DETECTORS` 各项 `name` 字符串与 `detector_order_is_frozen` 断言逐项一致（6 项）；`Worktree::new` 返回 `Option<Worktree>` 且 `resolve` 收 `Option<&Worktree>`（Task 12/13/14 的测试都按此签名写）。

**4. 范围检查。** 14 个任务、四周量级、单一交付物（一个能读 diff 并输出结构事件的二进制 + 一套金标准）。范围与 spec §17 的 W1 对齐，未夹带 W2–W4 的活。

---

## 执行者需要注意的四处判断

1. **金标准期望值不得由实现输出反填。** Task 8/9/10/11 都要落 `.events.json`。正确流程是：先按 spec §5.1 想清楚"这个改动该产出什么"，写下期望，跑一次，**不一致时判断谁错了**。直接 `--dump-events > expect.json` 会让测试变成实现的影子，从此任何回归都测不出来（这是 spec R1 与"负对照"纪律的共同要求）。

2. **Task 6 Step 2 的人工核对不能跳。** kinds.tsv 里出现的每个 node kind 都要么进五张表之一，要么在 `NON_EVENT_KINDS` 里被显式承认。`every_kind_is_classified` 会在将来 grammar 升级引入新 kind 时失败——那时是提醒，不是障碍，处理方式还是回到这一步重新核对。

3. **W3 开工前必须给 `Event` 加 `subject: Option<String>`。** 这是已知缺口，不是疏漏：`is_third_party_module` 判的是模块名，但 `Event` 现在只带 `node_kind`（`import_statement`）与 `range`（行号），**W3 的 HP1 拿不到它必须拿的东西**。`subject` 装模块名 / 参数名 / 被调函数名。因为 `--dump-events` 不属 `diffintent/1` 契约（spec §2 已注明 unstable），加这个字段不算破坏性变更；但 W1 内不加——加了没有消费者，只会让五个金标准 case 无谓重写。**这条写进 W3 计划的第一项。**

4. **`Worktree::read` 的 `canonicalize` 在 Windows 上会解符号链接**，若 `--repo` 指到一个含 junction/symlink 的目录，`starts_with(root)` 的判定可能反直觉地拒绝合法路径。Task 5 Step 5 的 `refuses_paths_escaping_the_worktree` 只测了"该拒的拒了"，**没测"不该拒的没被拒"**——W1 内补一条正向测试（仓库内子目录 + 一个指向同目录的符号链接，Windows 上用 `\\?\` 前缀或 junction 造），因为这条在 CI 的 Windows job 上才会暴露，而它坏了的表现是"所有文件都降到 hunk_only"，静默且致命。





