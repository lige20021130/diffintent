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
