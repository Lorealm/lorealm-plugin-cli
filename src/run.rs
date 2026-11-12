use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Runs `program` in `dir`, showing its output. Fails if it does.
pub fn run(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let line = format!("{program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("couldn't run `{line}`"))?;
    if !status.success() {
        bail!("`{line}` failed");
    }
    Ok(())
}

/// What `program` prints in `dir`, trimmed. Fails if it does, with what it
/// printed as the reason.
pub fn output(dir: &Path, program: &str, args: &[&str]) -> Result<String> {
    let line = format!("{program} {}", args.join(" "));
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("couldn't run `{line}`"))?;
    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr);
        bail!("`{line}` failed: {}", reason.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
