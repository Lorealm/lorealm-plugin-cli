use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Runs `program` in `dir`, showing its output. Fails if it does.
pub fn run(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let line = format!("{program} {}", args.join(" "));
    let status = Command::new(resolve(program))
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
    let output = Command::new(resolve(program))
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

/// `program` as a path `Command` can start. On Windows, `Command` only finds
/// `.exe`s by bare name, so `npm` has to be found as `npm.cmd` here.
fn resolve(program: &str) -> PathBuf {
    if cfg!(windows) {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let extensions = std::env::var("PATHEXT").unwrap_or_default();
        if let Some(found) = find(
            program,
            &std::env::split_paths(&path).collect::<Vec<_>>(),
            &extensions,
        ) {
            return found;
        }
    }
    PathBuf::from(program)
}

/// The first `program` with one of `extensions` (as in `PATHEXT`, such as
/// `.EXE;.CMD`) in `dirs`. A program named with an extension or a path is
/// left to `Command`.
fn find(program: &str, dirs: &[PathBuf], extensions: &str) -> Option<PathBuf> {
    let name = Path::new(program);
    if name.extension().is_some() || name.components().count() != 1 {
        return None;
    }
    let extensions = if extensions.is_empty() {
        ".COM;.EXE;.BAT;.CMD"
    } else {
        extensions
    };
    dirs.iter().find_map(|dir| {
        extensions
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| dir.join(format!("{program}{}", extension.to_lowercase())))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_name_is_found_with_its_extension() {
        let dir = std::env::temp_dir().join(format!("lorealm-plugin-find-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("npm.cmd"), "").unwrap();
        let dirs = [dir.clone()];

        assert_eq!(find("npm", &dirs, ".EXE;.CMD"), Some(dir.join("npm.cmd")));
        assert_eq!(find("npm", &dirs, ""), Some(dir.join("npm.cmd")));
        assert_eq!(find("npm", &dirs, ".EXE"), None);
        assert_eq!(find("npm.cmd", &dirs, ".CMD"), None);
        assert_eq!(find("cargo", &dirs, ".EXE;.CMD"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
