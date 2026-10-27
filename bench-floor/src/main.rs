#![forbid(unsafe_code)]

//! 冷启动参考下界。这个二进制什么都不做，只负责"进程从零到退出要多久"。
//! scripts/check-budget.sh 用它的启动时间减掉同样的启动时间，得到 diffintent 的边际成本。
fn main() {}
