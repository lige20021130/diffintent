#![forbid(unsafe_code)]

//! 本地开发命令。绝不进发布物、不进 README 的安装路径。
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Xtask {
    #[command(subcommand)]
    cmd: Cmd,
}

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
        // 真实实现在 Task 3 落地并会整体重写此分支。
        Cmd::Gendev => {
            println!("implemented in Task 3");
            Ok(())
        }
        Cmd::Calibrate => Err("W4 实现（spec §8.2）".into()),
        Cmd::Budget => Err("Task 14 接线".into()),
    }
}
