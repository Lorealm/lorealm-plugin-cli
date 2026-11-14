use std::path::Path;

use anyhow::{Result, bail};

use crate::project::Project;
use crate::run::{output, run as run_command};

/// The target plugins are built for.
const TARGET: &str = "wasm32-wasip2";

/// What's missing, as lines to show, and whether any of it stops a plugin
/// from being built. `npm` is for plugins whose frontend npm builds.
pub fn missing(fix: bool, npm: bool) -> Result<(Vec<String>, bool)> {
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
    if npm && !has("npm") {
        lines.push(
            "Node isn't installed, and this plugin's frontend is built with npm. Get it from https://nodejs.org"
                .to_owned(),
        );
        blocking = true;
    }
    if !has("git") {
        lines.push("git isn't installed. `stage` and `publish` need it".to_owned());
    }
    Ok((lines, blocking))
}

pub fn run(fix: bool) -> Result<()> {
    // In a plugin's folder, what that plugin is built with counts too.
    let npm = Project::open(".").is_ok_and(|project| {
        project.manifest.build.frontend.first().map(String::as_str) == Some("npm")
    });
    let (lines, blocking) = missing(fix, npm)?;
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
