// 性能回归骨架（spec §12：50 文件 / 2000 行 < 300 ms，CI 判定 600 ms）。
//
// 现状：整个测试体是注释，本文件因此**不贡献任何测试**。这不是遗漏，而是 Task 1 的事实——
// 要调用的三个函数此刻都还不存在：`patch::parse` 在 Task 3 落地、`events::extract_events`
// 在 Task 5/7 落地（模块是 src/events/，不在 src/seir/）、`evidence::resolve` 在 Task 8 落地。
// src/lib.rs 今天只声明 contract/evidence/patch/seir 四个文档桩模块。
//
// CI 的"性能基准"步骤在测试补上之前必须**失败**（复核 MB1a）：它先用
// `cargo test --release --locked --workspace -- --ignored --list`
// 钉住 `fifty_file_two_thousand_line` 这个名字确实存在；名字不在列表里就 exit 1。
// 否则 `--ignored` 跑 0 个测试也 exit 0，等于在 CI 里宣传不存在的覆盖。
// Task 14 取消注释、解除 ignore 后，该步骤会自然转绿——不需要再改 ci.yml。
//
// 解除注释时有两处签名以计划裁定为准，与下面留存的旧写法不同，别照抄：
//   - R2：`extract_events(...)` 返回 `ExtractOutcome { events, degradations, resolved }`，
//         不是 `Vec<Event>`；计数取 `.events.len()`。
//   - R3：`evidence::Worktree::none()` 已删除，"无工作树"统一传 `None`
//         （`resolve(files, Option<&Worktree>)`）。
//
// 旧的 `#[ignore]` 理由写着"由 scripts/check-budget.sh 调用"，不属实：
// check-budget.sh 只判体积与冷启动，从不跑 cargo test。真实调用方是 ci.yml 的性能步骤。
/*
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
#[ignore = "需 --release；由 .github/workflows/ci.yml 的\"性能基准\"步骤以 -- --ignored 调用（不是 check-budget.sh）；Task 14 解除"]
fn fifty_file_two_thousand_line_under_600ms() {
    let text = synthetic_diff(50, 20); // 50 文件 × 20 行 × 双向 ≈ 2000 行变更
    let start = Instant::now();
    let parsed = diffintent::patch::parse(&text).expect("parse");
    let events = diffintent::events::extract_events(&parsed, None).expect("extract");
    let ms = start.elapsed().as_millis();
    println!("50f/2k diff: {ms} ms, {} events", events.len());
    assert!(ms < 600, "超预算: {ms} ms");
}
*/
