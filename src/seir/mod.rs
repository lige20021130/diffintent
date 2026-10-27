//! 结构事件中间表示（SEIR）。EventKind 枚举与序列化自 Task 5 在此落地。

/// spec §5.1 表格顺序。用于 --dump-events 与文档一致性断言。
pub const EVENT_KIND_NAMES: [&str; 14] = [
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
