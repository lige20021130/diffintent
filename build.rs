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
        let src_dir = Path::new(
            &env::var("CARGO_HOME")
                .unwrap_or_else(|_| format!("{}/.cargo", env::var("HOME").unwrap_or_default())),
        )
        .join("registry/src")
        .join(format!("{krate}-{ver}"));
        if !src_dir.exists() {
            return Err(format!("grammar src missing: {}", src_dir.display()).into());
        }
        let dst_dir = Path::new(&out_dir).join("grammar-src").join(lang);
        fs::create_dir_all(&dst_dir)?;
        for f in [
            "parser.c",
            "scanner.c",
            "scanner.h",
            "allocarray.c",
            "ucd_pattern.c",
            "array.h",
        ] {
            let from = src_dir.join("src").join(f);
            if from.exists() {
                fs::copy(&from, dst_dir.join(f))?;
            }
        }
        // 让语法 crate 的 build script 使用本地副本而非下载
        println!(
            "cargo:rustc-env=DIFFINTENT_GRAMMAR_SRC_{lang}= {}",
            dst_dir.display()
        );
    }
    Ok(())
}

fn parse_locked_version(lock: &str, krate: &str) -> Option<String> {
    let mut in_block = false;
    for line in lock.lines() {
        if line.trim() == "[[package]]" {
            in_block = false;
            continue;
        }
        // 行为与 let-chain 版本一致；edition 2021 不支持 let chains，故用嵌套 if-let。
        if in_block {
            if let Some(v) = line
                .strip_prefix("version = \"")
                .map(|s| s.trim_end_matches('"').to_string())
            {
                return Some(v);
            }
        }
        if line == format!("name = \"{krate}\"") {
            in_block = true;
        }
    }
    None
}

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
        assert_eq!(
            parse_locked_version(lock, "tree-sitter-python").as_deref(),
            Some("0.25.0")
        );
        assert_eq!(
            parse_locked_version(lock, "serde").as_deref(),
            Some("1.0.219")
        );
    }
    #[test]
    fn returns_none_for_absent_package() {
        assert_eq!(
            parse_locked_version("name = \"x\"\n", "tree-sitter-go"),
            None
        );
    }
}
