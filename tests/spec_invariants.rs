//! spec 红线的可执行断言（复核 MB1b）。
//!
//! 在 Task 1 这个时点上，流水线还没有任何行为可测，`cargo test` 因此必须先钉住
//! "契约形状"本身：段名序、事件类型名序、已实现段数。这三条一旦漂，后面的任务
//! 会把漂移当成既成事实带进 JSON 契约，所以顺序逐位比较，不比集合、不比长度。
//!
//! 期望值在这里**故意另抄一份**，不引用被测常量——引用自身就等于没有断言。
//! 改动这些名字的人必须同时改动这里，PR 里就是一次显式的契约变更。

/// spec §5.1 表格顺序；Task 2 的 `EventKind` 枚举以此为准生成。
const EXPECTED_EVENT_KIND_NAMES: [&str; 14] = [
    "signature",
    "guard",
    "control_flow",
    "error_handling",
    "expression",
    "formatting",
    "comment",
    "binding",
    "call",
    "dependency",
    "module",
    "file",
    "test_case",
    "config_key",
];

/// spec §4 的七段流水线顺序。
const EXPECTED_PIPELINE_STAGES: [&str; 7] = [
    "patch_parse",
    "evidence_resolve",
    "event_extract",
    "block_segment",
    "prior_score",
    "drift_align",
    "render",
];

#[test]
fn event_kind_names_are_the_spec_table_in_order() {
    assert_eq!(
        diffintent::seir::EVENT_KIND_NAMES,
        EXPECTED_EVENT_KIND_NAMES,
        "EVENT_KIND_NAMES 的名字或顺序偏离 spec §5.1；它是 --dump-events 的规范序，改名/换位都是契约破坏"
    );
    assert_eq!(diffintent::seir::EVENT_KIND_NAMES.len(), 14);
    let mut sorted: Vec<&str> = diffintent::seir::EVENT_KIND_NAMES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 14, "EVENT_KIND_NAMES 有重名项");
}

#[test]
fn pipeline_stages_are_seven_in_spec_order() {
    assert_eq!(
        diffintent::PIPELINE_STAGES,
        EXPECTED_PIPELINE_STAGES.as_slice(),
        "PIPELINE_STAGES 偏离 spec §4 的七段顺序"
    );
    assert_eq!(diffintent::PIPELINE_STAGES.len(), 7);
}

#[test]
fn implemented_stage_prefix_is_three() {
    let implemented = diffintent::IMPLEMENTED_STAGES;
    let stages = diffintent::PIPELINE_STAGES;
    assert_eq!(
        implemented, 3,
        "W1 只实现 patch_parse/evidence_resolve/event_extract 三段；改这个数字要连同 --json 的能力边界标注一起改"
    );
    // 取 (0, 7) 的真前缀：0 意味着 --json 声称什么都没做，== len 意味着假装完整。
    assert!(
        implemented > 0 && implemented < stages.len(),
        "IMPLEMENTED_STAGES={implemented} 必须是 (0, {}) 内的前缀长度，越界即空切片或假装完整",
        stages.len()
    );
}
