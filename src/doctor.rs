use std::path::Path;

use anyhow::{Result, bail};

use crate::run::{output, run as run_command};

/// The target plugins are built for.
const TARGET: &str = "wasm32-wasip2";

/// What's missing, as lines to show, and whether any of it stops a plugin
/// from being built.
pub fn missing(fix: bool) -> Result<(Vec<String>, bool)> {
    let here = Path::new(".");
    let has = |program: &str| output(here, program, &["--version"]).is_ok();
    let mut lines = Vec::new();
    let mut blocking = false;

    if !has("cargo") || !has("rustup") {
        lines.push("Rust isn't installed. Get it from https://rustup.rs".to_owned());
        blocking = true;
    } else {
        let installed = output(here, "rustup", &["target", "list", "--installed"])?;
        if !installed.lines().any(|t| t == TARGET) {
            if fix {
                run_command(here, "rustup", &["target", "add", TARGET])?;
            } else {
                lines.push(format!(
                    "The Rust target `{TARGET}` isn't installed. Add it with `rustup target add {TARGET}`, or `lorealm-plugin doctor --fix`"
                ));
                blocking = true;
            }
        }
    }
    if !has("git") {
        lines.push("git isn't installed. `stage` and `publish` need it".to_owned());
    }
    Ok((lines, blocking))
}

pub fn run(fix: bool) -> Result<()> {
    let (lines, blocking) = missing(fix)?;
    for line in &lines {
        println!("{line}");
    }
    if blocking {
        bail!("plugins can't be built yet");
    }
    if lines.is_empty() {
        println!("Everything for building and releasing plugins is installed.");
    }
    Ok(())
}
