use clap::Parser;

#[derive(Parser)]
#[command(
    name = "diffintent",
    version,
    about = "intent-level diff analysis, zero LLM by construction"
)]
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
