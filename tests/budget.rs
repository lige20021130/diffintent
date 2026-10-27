// 性能回归骨架（spec §12：50 文件 / 2000 行 < 300 ms，CI 判定 600 ms）。
// TODO(Task 3/Task 8)：diffintent::patch / diffintent::events 落地后取消注释本测试体。
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
*/
