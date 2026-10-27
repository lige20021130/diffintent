# diffintent — 设计文档

状态：已与用户逐段确认，待复核
日期：2026-10-27
范围：MVP（v0.1）

**章节引用约定**：写作 `原书 §x` 的是用户提交的 diff-intent 项目书章节；无 `原书` 前缀的是本文档自身章节。

---

## 0. 一句话

**把 diff 从"文本差异"压成"意图差异"**：一个构造上不可能调用网络的 Rust CLI，读入 unified diff，输出每个变更块的意图标签、一句话 reason、校准过的置信度，以及三态 drift 判定。

---

## 1. 决策记录

这十一条是本设计的基石，每条都在 brainstorming 阶段由用户拍定或由证据强制。

| # | 决策 | 依据 |
|---|---|---|
| 1 | 名称 `diffintent`（不是 `diff-intent`） | **强制**。`diff-intent` 在 npm 已被占（v1.0.2），且 GitHub 上 4★ 的 `SyedSibtainRazvi/diff-intent` 简介原文就是 "AI-powered git diff analysis – understand the intent behind code changes"。`diffintent` 是五个候选里唯一在 npm / PyPI / crates.io 三个注册表全空的 |
| 2 | 产品本体 = engine（精度 + 可复现）；传播面 = agent 消费 | 用户先选 Engine 优先；随后证据显示同赛道天花板约 50★（intentdiff 3★ / diffscope 13★ / drift 51★），而涨 star 由消费面决定（rtk 84k★ 全通道、agent-browser 44k★ 走 npm）。最终定为"守 engine，换消费面" |
| 3 | 实现语言 Rust | 用户选定。tree-sitter 原生绑定、静态语法包、跨平台预编译二进制 |
| 4 | 三级证据降级链 `full_ast → hunk_only → text_fallback` | unified diff 是有损输入（每 hunk 仅 ±3 行上下文）。把原书"语言维度的降级"扩成"证据维度 × 语言维度"双轴 |
| 5 | 架构 = 七段线性流水线 + 单一 `seir` IR | 相比 detector trait 注册表，静态顺序是"同 diff 同输出"这条硬指标的唯一保障 |
| 6 | drift 改三态 `true \| false \| null`，原书 §7 验收口径随之重定义 | 原书 `召回≥0.8` 与 `精确≥0.7` 同成立，跟原书 §8.2"误报比漏报更糟"是相反方向的力。三态让两类错误各归其位 |
| 7 | **LLM 兜底层整块删除**，不留 flag、不留 HTTP 依赖 | 它同时打破"确定性输出""≤3 依赖""代码不出本机"三条硬指标，而按原书 §5.3 它永远不能参与 drift 否决，唯一作用是美化 `unknown` 的措辞——价值载荷为零 |
| 8 | 评测集 = 合成注入（供 drift）+ 真实 agent 会话（供意图三分类） | drift 正样本在真实数据中极稀缺；合成注入的标签由构造本身给出且可复现 |
| 9 | 首版即带退出码契约 + composite `action.yml` + 四通道安装 | `git diff \| diffintent` 只能"看"，不能"拦"，则原书 §1 的"管道友好"是空卖点 |
| 10 | 输入 flag 由原书的 `--intent` 改名 **`--request`** | `intent` 是**输出**字段名（七类枚举）。同一个词既指输入又指输出，README 和 SKILL.md 里必然写出"`--intent` 设置 `intent`"这种句子；更糟的是 `--intent "refactor"` 会被误读成"我在断言这是重构"而非"我在提供锚点"。改名后 `request`/`intent` 的输入输出关系一眼分明，且 JSON 里 `request` 字段本来就叫这个 |
| 11 | 事件表比原书细一层：新增 `guard`、`file`、`comment` 三个 kind | `guard` 让原书 §4.2 自己的示例（"在 token 查找前加空值守卫"）能被精确表达——那是 guard，不是笼统的 control flow；`file:removed` 是最强的 refactor/cleanup 信号，原书漏了；`comment` 必须折叠否则注释增删会污染 `expression` 统计 |

---

## 2. 输入与 CLI 表面

```bash
git diff                | diffintent
git diff --staged       | diffintent --request "fix null check in login"
diffintent --file patch.diff --request "add rate limiting" --json
diffintent --repo /path/to/tree            # 显式指定重建基线的工作树
```

| 表面 | 说明 |
|---|---|
| 位置/stdin | unified diff 为必需输入；`--file` 可从文件读 |
| `--request <str>` | 原始请求摘要。**有它才可能产出 `drift: true`**；无它只产 `structural_anomaly` |
| `--repo <path>` | 重建完整新旧文件的基线工作树。默认取 cwd |
| `--json` | 输出 `diffintent/1` 契约。与终端输出同源，不允许两套渲染逻辑 |
| `--fail-on-drift` | 存在 `drift: true` 块时退出码 1 |
| `--fail-on-unknown` | 存在 `confidence < 0.6` 或 `unknown` 块时退出码 2 |
| `--details` | 逐行内容。默认一屏摘要，不展示逐行 |
| 无网络参数 | **不存在 `--llm-fallback`、`--api-key`、`--model` 这类选项** |

### 2.1 退出码（版本化契约的一部分）

| 码 | 含义 |
|---|---|
| 0 | 正常，无 drift true、无低置信块 |
| 1 | 存在 `drift: true` 块（需 `--fail-on-drift`） |
| 2 | 存在 `confidence < 0.6` 或 `unknown` 块（需 `--fail-on-unknown`） |
| 3 | 输入非法 / 解析失败 / 非 unified diff |

默认不带 flag 时恒为 0（除崩溃）。退出码语义的变更属破坏性变更，需升契约版本号。

---

## 3. 输出契约 `diffintent/1`

```json
{
  "schema": "diffintent/1",
  "request": "fix null check in login",
  "request_lang": "en",
  "summary": {
    "files_changed": 3,
    "evidence_tiers": { "full_ast": 2, "hunk_only": 1, "text_fallback": 0 },
    "structural_events": {
      "guard": 1, "control_flow": 2, "error_handling": 1,
      "expression": 7, "formatting": 4
    },
    "dominant_intent": "fix",
    "intent_distribution": { "fix": 0.62, "refactor": 0.21, "drift": 0.17 },
    "drift_risk": "medium",
    "drift_check": "enabled",
    "structural_anomalies": [],
    "confidence": 0.71
  },
  "blocks": [
    {
      "file": "src/auth.ts",
      "lines": [12, 48],
      "scope": "authenticate",
      "intent": "fix",
      "reason": "adds null guard before token lookup",
      "confidence": 0.84,
      "evidence_tier": "full_ast",
      "structural_events": ["guard:added", "control_flow:added"],
      "drift": false,
      "evidence": { "node_kind": "if_statement", "lines": [14, 22] }
    },
    {
      "file": "src/metrics.ts",
      "lines": [3, 61],
      "scope": null,
      "intent": "drift",
      "reason": "new module outside the request's anchored paths",
      "confidence": 0.66,
      "evidence_tier": "full_ast",
      "structural_events": ["module:added"],
      "drift": true,
      "drift_trigger": "HP2",
      "evidence": { "node_kind": "class_declaration", "lines": [3, 5] }
    }
  ],
  "unknown": [
    {
      "file": "src/util.ts",
      "lines": [88, 95],
      "reason_code": "no_ast_match",
      "detail": "parser fallback: no AST match"
    }
  ]
}
```

（示例只展示了 2 个 block，而 `intent_distribution` 是对全部 block 求的，见 3.1 的口径定义。）

### 3.1 契约纪律

- `intent` 枚举固定七类：`add | refactor | fix | config | test | drift | unknown`
- `drift` 三态：`true | false | null`。`null` = "结构上超出请求期望，但请求语义不足以裁定"，归入人工确认区，不背精确率的锅
- 每个 block 必须携带 `evidence_tier`；`summary.evidence_tiers` 让人一眼看到这份结论里有多少是低证据推断。**任何渲染都不得隐藏 tier**
- `structural_events` 采用 `kind:direction` 字符串形式（如 `error_handling:removed`），方向不可丢失
- `drift_risk` 与 `drift` 仅在 `request` 存在时输出；否则 `drift_risk: null`、`drift_check: "skipped_no_request"`
- `unknown[].reason` 必须是枚举码（`no_ast_match` / `margin_below_threshold` / `request_anchor_missing` / `grammar_unsupported`），不得为自由文本——否则无法统计"unknown 占比 ≤15% 且可解释"
- `schema` 版本号变更 = 破坏性变更，需在 CHANGELOG 显式标记

### 3.1.1 summary 字段口径（原书未定义，此处钉死）

| 字段 | 口径 |
|---|---|
| `intent_distribution` | 分母 = **全部已分类 block**（不含 `unknown`，不含被折叠的 formatting/comment）。`drift` 作为意图项计入的是 `intent == "drift"` 的 block，与 `drift: true` 的三态标记**不是同一件事**（见 3.1.2） |
| `dominant_intent` | `intent_distribution` 的 argmax；并列时取 `margin` 更高者，仍并列则按枚举声明序取第一个（保证确定性） |
| `confidence`（summary 级） | block `confidence` 的**按 block 行数加权平均**；不用未加权平均，否则 3 行块与 300 行块同权 |
| `evidence_tiers` | **按文件计数**（tier 解析发生在文件粒度），不是按 block。故 `full_ast + hunk_only + text_fallback == files_changed` |
| `drift_check` | 枚举：`enabled`（有 request 且词典命中锚点）/ `skipped_no_request`（无 request）/ `no_anchor`（有 request 但 `anchored_files` 为空，此时所有 drift 为 `null`） |
| `structural_anomalies` | `drift_check != "enabled"` 时唯一可用的信号数组：`{file, lines, kind, direction, note}`。**它永不含 "drift" 字样**——这是原书 §5.3 硬纪律的契约级落实 |

### 3.1.2 `intent: "drift"` 与 `drift: true` 的关系

两者正交，且必须同时可 `true`/`false` 地组合，否则契约自相矛盾：

- `intent` 回答"这块在干什么"，来自结构事件先验，**无 request 也能给**
- `drift` 回答"这块是否超出请求期望"，来自 9.x 的锚点+HP 判定，**有 request 才能给**

一个 block 可以是 `intent: "add", drift: true`（合法的新增，但请求没要求）——这比原书示例的 `intent: "drift"` 更可解释。`intent: "drift"` 保留给**结构上无法归入其余六类**的块，属少数情况；评测时它不进三分类，只进 `unknown` 邻近统计。

### 3.2 人类可读输出（默认）

```text
intent: fix (confidence 0.71, full_ast 2/3)   request: "fix null check in login"

  fix      src/auth.ts:12-48   authenticate — adds null guard before token lookup
  drift?   src/metrics.ts:3-61 (new module) — outside request's anchored paths

  7 expression-only changes collapsed
  4 formatting changes collapsed
  1 block needs manual review (low confidence)
```

原则同原书：**默认不展示逐行；逐行是 `--details` 的事。**

---

## 4. 架构：七段流水线

段间只传两种类型：`seir::Event` 与 `Block`。每段是纯函数，可独立 golden test。

```
1 patch_parse       unified diff → Vec<Hunk>（行号/前后缀/文件元数据全保留）
2 evidence_resolve  每个文件定层级：full_ast | hunk_only | text_fallback
3 event_extract     新旧 → Vec<seir::Event>，带 tier
4 block_segment     跨 hunk 按 scope 包含关系合并 → Vec<Block>   ← 最难的一段
5 prior_score       声明式规则表 → intent + margin
6 drift_align       request 词典 → 期望事件集 → 三态 drift
7 render            terminal / JSON（同一棵内部树，不允许两套渲染）
```

### 4.1 确定性不变量（可测）

进入第 5 段前的所有集合必须按 `(file, range.start, kind, direction)` 规范排序。**打分与 drift 判定路径禁止依赖 HashMap 迭代序**——写成 property test：同一输入跑两次、以及打乱输入文件顺序，输出必须逐字节相同。

### 4.2 检测器写法

检测器保持统一签名 `fn(&Scope, &mut EventSink)`，按固定顺序存在 `static` 数组中。这保留原书 §5.1"规则表可社区扩展"的意图（PR 里多改一行而非多一个文件），同时不引入 trait object 与注册序不确定这两个不确定性入口。

---

## 5. 结构事件模型

**两轴设计：kind × direction。** 原书 §5.1 同时存在 `error_handling_changed`（笼统）与 `control_flow_added`（带方向），不一致；且"删掉 try/except"是最该报的信号，用 `changed` 会和"加了 try/except"混成一个。

```rust
pub struct Event {
    pub kind: EventKind,
    pub direction: Direction,        // Added | Removed | Modified
    pub scope: Option<ScopeRef>,     // 所属定义：名字 + 节点类型 + 新文件行区间
    pub node_kind: String,           // tree-sitter 原始节点名 → 填入 evidence.node_kind
    pub range: (u32, u32),
    pub tier: EvidenceTier,          // FullAst | HunkOnly | TextFallback
    pub file: FileId,
}
```

### 5.1 MVP 的 14 个 kind

| kind | 说明 | 主要意图先验 |
|---|---|---|
| `signature` | 参数增/删/改名、默认值、返回类型 | `add` 或 `refactor`，按子类型分流 |
| `guard` | 函数入口的早退 / 空值判断 / assertion | `fix` ↑↑（新增 kind，原书无） |
| `control_flow` | if/for/while/match 增删 | `fix` ↑ / `add` ↑ |
| `error_handling` | try/catch/throw/raise | Added→`fix` ↑↑；**Removed→独立风险信号** |
| `expression` | 表达式改写，结构不变 | `refactor` ↑，折叠 |
| `formatting` | token 流完全相同 | **折叠，不产生意图** |
| `comment` | 注释 / 文档 | 折叠 |
| `binding` | 新变量 / 常量 | 弱 |
| `call` | 被调函数名或实参变化 | 中性，供 reason 文本 |
| `dependency` | 新增第三方 import | **drift HP1** |
| `module` | 整新文件且内含定义 | **drift HP2** |
| `file` | 文件增删 | `file:removed` → `refactor`/`add` 信号（原书无） |
| `test_case` | 测试用例增删改 | `test` ↑↑ |
| `config_key` | 配置键/值变更 | `config` ↑↑ |

推迟、不假装做：AST 级 `moved_or_renamed` 全量检测、非 TS/JS/Python 的结构事件、类型推断、调用方分析。

### 5.2 两个纯排除层（不需要 AST）

原书 §3 说"借用 intentdiff 的思路作为排除层"——本设计只借判据，不借它的分类体系：

- `formatting` = 新旧 **token 流逐位相等**
- `rename` = 直接读 git diff 的 `rename from/to` 元数据行

两者折叠，不参与意图判断，不产生 drift 信号。

---

## 6. 三级证据降级链

原书只定义了"语言维度"的降级。这里补上"证据维度"，因为旗舰 UX（`git diff | diffintent`）拿到的 unified diff 每个 hunk 只带 ±3 行上下文，函数签名在 hunk 外就拿不到 `scope`，孤立 hunk 里看不出这是不是整个新文件带来的 `module`。

| 层级 | 触发条件 | 能力 |
|---|---|---|
| `full_ast` | cwd（或 `--repo`）是 git 工作树 **且** diff 的 `+++ b/` 路径在磁盘存在 **且** 内容校验对得上 | 重建完整新旧文件 → 双侧真 AST → `scope` 可靠、`module`/`dependency` 判据完整 |
| `hunk_only` | 上条任一不成立 | 每个 hunk 的新旧两侧按代码片段独立解析 → 对比事件集。`scope` 常为 `null` |
| `text_fallback` | 语法不可用（非 TS/JS/Python）或解析失败 | 扩展名 + 关键字 + 行级模式；标 `grammar_unsupported`，**置信度按 tier 打折** |

内容校验**不得静默继续**——工作树与 diff 不同源（PR 分支、staged 与工作区不一致）时，全量 AST 比 hunk 片段更危险。校验方法（确定性、无需 git 对象库）：对每个 hunk，检查磁盘文件的**前镜像**（`context` 行 + `-` 删除行）是否与 hunk 声明的行位置逐行相符；**任一 hunk 不符即整个文件降到 `hunk_only`**，并在 `summary` 记录降级原因 `preimage_mismatch`。不做部分采信，因为半对半错的 AST 会产出看似高置信的错判。

**校准分桶按 tier 切**，这直接落实原书 §8.4："满血 AST"和"瞎猜 hunk"的置信度不该混在一个桶里算 Brier。

---

## 7. block 分段（第 4 段）

这是 MVP 里最难的一段，也是"一 hunk 即一 block"方案被否掉的原因。

- 所有 `scope` 解析到**同一 enclosing definition** 的事件 → 并成一个 block
- 无 `scope` 的事件（顶层 import、module 级语句）→ 按文件归到一个 `scope: null` 的 block
- **跨文件永不合并**
- `lines` = 成员事件行区间的 min..max

AI 加一个功能通常是三个 hunk（新 import、新函数、一处调用）。不合并则意图信号碎成三份、每份置信度被稀释、`module:added` 这类 drift 关键信号永远落在孤立 hunk 上看不全。本文 §3 契约里的 `lines: [12, 48]` 就是这个合并的结果。

---

## 8. 规则表、打分与置信度

### 8.1 规则表是数据

```rust
pub struct Rule {
    pub when: &[Pattern],          // 匹配 (kind, direction, tier, scope_kind)
    pub prior: (Intent, i16),      // 意图 + 权重增量
    pub note: &'static str,
}
```

`static` 数组，可单测、可 diff review、版本化。本文 §5.1 的表就是它的初始内容。

### 8.2 置信度不由规则表直接给出

打分得 `argmax intent` + `margin`。**margin 绝不直接当 `confidence` 输出**——那正是原书 §8.4 警告的"拍脑袋 0.9"。改为查表：

```
confidence = CALIBRATION[tier][margin_bucket]
```

`CALIBRATION` 是**从评测集生成、带 eval-set 版本号、提交进仓库**的静态表，值即"该桶实际判对的比例"。任何人 clone 后重跑评测可重算出同一张表。

于是"校准"从原书 §7 的一个验收项变成产品的**构建产物**：`eval/` 目录 + `cargo xtask calibrate` + `src/calibration.rs`（生成物，人不手改）。这是与三个直接竞品的实质差异点——它们都没有可信的置信度数字。

### 8.3 拒判优于猜

`margin` 低于阈值 → 进 `unknown`，`reason_code = margin_below_threshold`。低置信度是特性不是失败；不为凑指标把模糊块硬编成高置信度标签。

---

## 9. drift 三态

**设计原则一句话：`true` 的精确率不靠调阈值，靠限制产生器。** 只有四条通道能到达 `true`，其余一律只能到 `null`。

### 9.1 第一步：request → 期望事件集

确定性词典，MVP 只做英文（`request_lang` 字段显式声明）：

| 触发词 | IntentHint | 期望 kind |
|---|---|---|
| fix / bug / crash / null / exception / error | `fix` | `guard`, `error_handling`, `control_flow` |
| add / support / implement / introduce / feature | `add` | `module`, `signature`, `binding`, `dependency`, `config_key` |
| refactor / rename / extract / simplify / clean | `refactor` | `expression`, `formatting`, `signature`, `comment`, `file` |
| test / coverage / spec / assert | `test` | `test_case` |
| config / env / deploy / ci / flag / setting | `config` | `config_key`, `dependency` |
| remove / delete / drop / deprecate | `refactor`/`add` | `file`(Removed), `binding`(Removed) |
| perf / optimize / cache | `fix`/`refactor` | `expression`, `control_flow` |

这是先验，不是语义理解。词典是数据文件，版本化。

### 9.2 第二步：锚点解析（原书 §8.2 的命门）

从 request 抽取"位置名词"（标识符、路径样 token、`login`/`auth`/`rate limit` 这类归一化名词），匹配**文件路径分段**与 **diff 中的 definition 名**，得出 `anchored_files`（可以为空）。

判定表（每 block）：

```
无 request                       → 不输出 drift；summary.drift_check = "skipped_no_request"
                                   只报 structural_anomaly
request 有词但锚点为空            → drift: null, reason_code = request_anchor_missing
锚点内 且 kinds ⊆ 期望            → drift: false
锚点内 但 kinds ⊄ 期望            → drift: null        （语义不足以裁定，不升级）
锚点外 且命中 HP 触发器            → drift: true        （唯一能报 true 的通道）
锚点外 未命中 HP                  → drift: null
```

### 9.3 四条 HP（高精确）触发器

| 编号 | 条件 |
|---|---|
| HP1 | `dependency:added` 且同时满足三者：(a) import 判定为第三方——**相对路径与项目别名前缀（`@/`, `~/`, workspace 名）算本地，不触发**；(b) 归一化后的包名不出现在 request 中；(c) request 的期望 kind 集合**不含** `dependency`。（(c) 是关键：request 写 "add rate limiting" 时 `add` hint 的期望集含 `dependency`，新增限流依赖不报 drift，这是合法行为而非漏洞。） |
| HP2 | `module:added`，文件同时满足：不在 `anchored_files`、不在锚点文件的同目录、不属于 test/config 目录 |
| HP3 | `error_handling:removed` — **无条件 true，即使有锚点。** 唯一破例：删错误处理是行为风险，不是意图问题 |
| HP4 | `config_key:added/modified` 而 request 无任何 `config` hint（偷改配置是 agent 越界最典型的形状） |

block 上输出 `drift_trigger: "HP1"\|"HP2"\|"HP3"\|"HP4"`，供误报归因统计。

### 9.4 反噬装置：`derived_path` 分类器

`package-lock.json`、`pnpm-lock.yaml`、`yarn.lock`、`dist/`、`build/`、`*_pb2.py`、`.min.js`、`vendor/`、`__snapshots__`、`__snapshots__/*.snap` 这类路径是**合法变更的副产品，永远不得报 drift**。折叠进引起它的 block 并标 `note: "derived"`。

真实 AI diff 里 drift 误报的头号来源就是 lockfile。这条不进 MVP，第一条 issue 会是"它把我的 pnpm-lock 报成越界"。

### 9.5 `drift_risk`

`high`（≥1 个 true）/ `medium`（有超出期望的 null，或有锚点外新文件）/ `low`（全 false）/ `null`（无 request）。

---

## 10. LLM 层的删除与红线（代码化）

不留 flag、不留 HTTP 依赖、不留配置项。卖点从"默认零 LLM"升级为"**构造上不可能调网**"，并用两道门禁落实：

1. **CI 依赖图门禁**：`cargo tree` 结果中出现任何 HTTP/reqwest/isahc/ureq/hyper-client 类依赖即构建失败
2. **契约测试**：断言 `intent` / `drift` / `confidence` 三个字段在任何构建模式下都不可被非确定源写入

`unknown` 块保持诚实：只报 `{file, lines, reason_code, detail}`。开发者读 `parser fallback: no AST match` 得到的信息量，大于读一句圆润的猜测。

---

## 11. 评测集

两个集子，**base 仓库不得重叠**（防泄漏）。

### 11.1 S 集：合成注入（供 drift）

选 3–5 个 MIT/BSD 许可的小型 TS/JS + Python 仓库，冻结到确定 commit，作为 fixture 落进 `eval/base/` 并附许可署名。

每 case 构造固定：**1 个 request R → 实现它 → 记录 diff → 从注入目录加 K 个未请求改动 → 标签由构造本身给出。**

| 编号 | 注入 | 期望 |
|---|---|---|
| I1 | 新增 metrics/logging 模块 | `true`（HP2） |
| I2 | 引入一个第三方依赖 | `true`（HP1） |
| I3 | 改掉一个配置键 | `true`（HP4） |
| I4 | 删掉一个 try/except | `true`（HP3） |
| **I5** | **重构一个无关文件** | **`null`，绝不能是 `true`** |
| **I6** | **纯格式化扰动** | **无任何信号** |
| **I7** | **git rename 元数据改名** | **折叠，无信号** |

I5/I6/I7 是**负对照**，直接量出误报率（判据必须能变红）。

**变异测试**：删除某条 HP 触发器的代码后，对应的 I1–I4 必须从 `true` 掉到 `null` —— 证明是这条触发器在抓，而非别的东西顺手抓到。

规模：约 24 个 case × 2–4 条注入。

### 11.2 T 集：真实 agent 会话（供意图三分类）

从用户真实用过的 AI 编程会话中抽取（request 与 diff 均为真，锚点天然成立）。

标注协议四条，必须发表：

1. **标注单元是 block，但分段必须由人独立做**——不得用 diffintent 自己的 `block_segment` 产物，那是自证。人先按判断把 hunk 分组，再用**行区间重叠 ≥50%** 把工具 block 映射到人工标注区。匹配规则必须公开写出，否则 F1 无意义。
2. **七类闭集，但 `unknown` 不是人可给的标签。** 人必须表态，工具才允许弃权。这个不对称是"低置信度是特性"成立的前提。
3. 单人自标的可信度替代物：**发布一页标注决策树**（谁标、判据、争议裁决）+ **公开列出争议样本并从 F1 剔除**（并声明剔除）。用可审计判据替代 inter-annotator agreement。
4. 提交前做隐私清洗（组织名、路径、真实项目名），这一步显式检查。

**规模按桶估，不按条估。** 原书"20–50 个 diff"是按条数估的，但校准分桶是 三证据层 × 七意图，桶一碎每桶剩个位数、Brier 会抖。一个 diff 平均约 5 个 block，故真实需求为 **≥150 个标注 block**；README 按 block 报，不按 diff 报。

---

## 12. 验收标准（修订版）

| 项 | 阈值 | 相对原书的变化 |
|---|---|---|
| `fix`/`refactor`/`add` 三分类宏平均 F1 | ≥ 0.75 | 不变；样本单位改为 block，需 ≥150 标注 block |
| drift `true` 精确率 | ≥ 0.7 | 收窄定义：仅在 `true` 上计 |
| drift 召回（`true ∪ null` 均算命中） | ≥ 0.8 | **口径显式声明**，README 必须写明是 true∪null |
| **新增** drift `null` 占比 | 报告值，无阈值 | 衡量"我有多常拒判" |
| **新增** 四条 HP 触发器各自误报数 | 报告值 | 暴露哪条在吵 |
| `unknown` 占比 | ≤ 15%，且 `reason_code` 可解释 | 不变 |
| 置信度分桶校准误差（按 tier 分层） | ≤ 0.15 | 分桶维度增加 tier |
| 冷启动 | < 80 ms | 不变 |
| 二进制体积 | < 12 MB | 已核实可行：ast-grep（16k★，静态链接多语法）Linux zip 实测 8.45 MB；三个 WASM 语法约 3.4 MB |
| 50 文件 / 2k 行 diff | < 300 ms | 不变 |
| 确定性 | 同输入逐字节相同（property test） | 落实为测试 |
| 零网络 | 依赖图 CI 门禁 | 从"默认关闭"升级为"构造不可能" |

---

## 13. 分发矩阵

数据结论：**涨得动的全通道**（rtk 84,721★ = brew+winget+curl-sh，且因 crates.io 撞名把 cargo 降级；agent-browser 44,687★ = Rust 二进制走 npm），而 **cargo-dist 与 star 的相关性未获验证**（搜索 cargo-dist &gt;1000★ 只返回它自身 2,142★）。故 cargo 不作卖点。

| 通道 | 形态 | 优先级 |
|---|---|---|
| `curl -fsSL .../install.sh \| sh` | 探测 os/arch，拉 GitHub Release；**必须验签**（GitHub Artifact Attestations + manifest 签名校验） | 首屏第一行 |
| Homebrew tap | 独立 `homebrew-tap` 仓库，workflow 自动 bump | 首版 |
| winget | 向 `microsoft/winget-pkgs` 提 manifest PR，定时 PR 维持 | 首版（Windows 是主力开发平台） |
| npm `diffintent` | 薄壳，postinstall 拉平台二进制；提供 `DIFFINTENT_BIN` 环境变量绕过脚本，照顾 `--ignore-scripts` 沙箱 CI | 首版（agent 受众） |
| `cargo install diffintent` | crates.io 名字可用，白拿 | 安装表最后一行 |
| Nix / AUR / scoop | — | 显式推迟 |

发布 target：`darwin-aarch64`、`darwin-x86_64`、`linux-x86_64`（musl 静态）、`linux-aarch64`、`win-x86_64`。

无签名的 `curl \| sh` 在 2026 年会被开发者直接点名供应链异味，故验签不是可选项。

**单一真源纪律**：四个通道 = 四个版本同步点，对单人项目是真实的长期负担。缓解方式是唯一真源 = Release tag，tap formula 与 winget manifest 全部由 workflow 从 tag 生成，人不手改任何一个。

### 13.1 GitHub Action

`action.yml` 用 **composite action，不用 JS action**：直接 shell 调预编译二进制，不打包 node runtime，不破坏单二进制叙事。`--fail-on-drift` 天然成为 PR 上的红叉。这是"管道友好"从口号变成日常可见性的唯一路径。

### 13.2 SKILL.md（≤5KB）

三个直接竞品（intentdiff 3★ / diffscope 13★ / drift 51★）**均未提供 SKILL.md**，而 `topic:claude-skills` 仓库数从 2026 年前的 350 涨到 2026 年的 12,623（约 36 倍）。这是唯一还开着的通道，且不要求放弃 engine 定位——engine 是产品本体，SKILL.md 是分发面。

内容仍按原书四条纪律：何时触发、怎么读（只看 `drift_risk`/`intent`/`confidence`，不原样贴 JSON）、drift 必须显式列出、低置信度不自行判断。加第五条：**不得隐藏或忽略 `evidence_tier`**。

---

## 14. 测试策略

| 层 | 内容 |
|---|---|
| 单元 | 七段流水线每段的 golden test（fixture diff → 期望 `Event`/`Block`） |
| 契约 | `diffintent/1` JSON 快照测试 + `schema` 版本断言 |
| 属性 | 确定性：同输入跑两次、打乱文件顺序 → 逐字节相同；打分路径无 HashMap 迭代序 |
| 负对照 | I5/I6/I7 必须为 `null`/无信号（判据能变红） |
| 变异 | 移除单条 HP 触发器 → 对应 I1–I4 必须从 `true` 掉到 `null` |
| 降级 | `grammar_unsupported` 路径必须只降级不猜；工作树与 diff 不同源必须降 tier |
| 门禁 | CI 依赖图禁止 HTTP 客户端；`intent`/`drift`/`confidence` 不可被非确定源写入 |
| 性能 | 冷启动 <80ms、50 文件/2k 行 <300ms 的基准测试进 CI（带宽松阈值防噪声） |

---

## 15. 明确不做

viewer / diff 展示、PR bot 与 Web UI、历史存储与平台化、变更类型分类（属 intentdiff 的活）、LLM judge（任何形态）、多二进制拆分（sift 保持为内联模块）、MCP server、`moved_or_renamed` 的 AST 全量检测、非 TS/JS/Python 的 AST 支持、中文 request 的 drift 词典、调用方分析。

---

## 16. 风险

| # | 风险 | 性质 | 缓解 |
|---|---|---|---|
| R1 | **精度天花板在评测集存在之前是未知的。** 14 个 kind 全做完，三分类可能只有 0.6 左右 | 最大风险，且非工程风险 | 把 S/T 集第一版做在规则表写全**之前**，先量一次；不达标则回到本节重定契约 |
| R2 | **README 首屏 demo 可能落在最低证据层。** `git diff \| diffintent` 不带 repo → 大量 `hunk_only` → 首印象"老在 unknown" | 产品风险 | demo 必须在仓库内跑（拿 `full_ast`），且 tier 打在屏幕上；藏 tier 与"诚实降级"立身之本冲突 |
| R3 | **tree-sitter 的 C 编译在 Windows MSVC 上是真实痛点**（`cc` crate + SDK 依赖），而 Windows 是主力开发平台 | 工程风险 | CI 从第一天带 Windows target，不留到分发阶段才发现构建不过 |
| R4 | **原书 §7 内部矛盾**：`召回≥0.8 ∧ 精确≥0.7` 与原书 §8.2"误报更糟"方向相反 | 已由三态 + 限制产生器化解，但仍是取舍 | `true` 只能经四条 HP 通道产生；`null` 承担拒判，不背精确率 |
| R5 | 单人维护四安装通道 = 四个版本同步点 | 长期负担 | 唯一真源 Release tag，其余由 workflow 生成 |
| R6 | `eval/base/` 引入第三方仓库的许可与体积 | 合规风险 | 仅取 MIT/BSD、冻结 commit、附许可署名、只保留所需子集 |
| R7 | drift 词典是英文先验，中文/混排 request 直接失锚 | 已知限制 | `request_lang` 显式声明；无锚点即 `null`，绝不升级成 `true` |

---

## 17. 里程碑

四周串行，**每周一个独立可验证的产出**。范围对单一实现计划偏大，故实现阶段按里程碑拆成四份计划（W1/W2、W3、W4 各成一份，W2 结尾的基线测量是进入 W3 的闸门）。

**W1 — 输入与事件**
`patch_parse` → `evidence_resolve` → `event_extract`（TS/JS 先行，Python 次之）。
交付：`Event` 序列 + tier 判定正确，golden test 绿。

**W2 — 先量化，后写规则**（顺序按 R1 缓解措施调整）
S 集最小版（8–10 case，覆盖 I1–I7）+ T 集最小版（约 40 block，标注决策树一页）
→ **先跑一次基线**，确认 14 个 kind 是否足以支撑三分类 → 再定 `prior_score` 规则表与 `block_segment` 合并规则。

**W3 — drift 与契约**
`drift_align`（词典 + 锚点 + 四条 HP + derived_path）、`diffintent/1` 落地、`--json`、退出码。

**W4 — 校准与分发**
`xtask calibrate` 生成 `CALIBRATION`、T 集补到 ≥150 block、README 两张表、五 target 发布 + 四通道 + composite action.yml + SKILL.md。

注：里程碑四周，比原书三周多一周，多的那一周是给"先量化再写规则"和四通道分发——原书 Week 2 把评测集排在规则表之后，正好踩中 R1。

---

## 18. 未决项（进入实现计划前需闭合）

1. **Rust 工具链安装授权** —— 本机当前无 `cargo`。需装约 1–2GB 到 `~/.cargo`，Windows 上还需 MSVC 生成工具组件。影响用户机器，待显式批准后执行。
2. **S 集 base 仓库选型** —— 3–5 个 MIT/BSD 小型 TS/JS + Python 仓库，待定具体清单。
3. **T 集会话来源确认** —— 从本地 agent 会话历史抽取会读取真实项目代码，需确认范围与清洗边界。
4. **仓库归属** —— GitHub 用户名/org、`install.sh` 分发的域名或 `releases/latest/download` 直链形式。
