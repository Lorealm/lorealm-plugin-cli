use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use lorealm_plugin_manifest::{MANIFEST_FILE, Manifest};
use semver::VersionReq;
use toml_edit::DocumentMut;

use crate::run::run;

/// The crate plugins are written against. Its version is the plugin API's.
pub const SDK: &str = "lorealm-plugin";

/// A plugin's folder.
pub struct Project {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

impl Project {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        if !dir.join(MANIFEST_FILE).is_file() {
            bail!("there's no {MANIFEST_FILE} here; run this in a plugin's folder");
        }
        let manifest = Manifest::from_dir(&dir)?;
        Ok(Self { dir, manifest })
    }

    /// The project as it is on disk now.
    pub fn reopened(&self) -> Result<Self> {
        Self::open(&self.dir)
    }

    /// Runs the manifest's `[build]` commands.
    pub fn build(&self) -> Result<()> {
        let build = &self.manifest.build;
        for command in [&build.backend, &build.frontend] {
            if let Some((program, args)) = command.split_first() {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                println!("$ {program} {}", args.join(" "));
                run(&self.dir, program, &args)?;
            }
        }
        Ok(())
    }

    /// Fails with everything that would stop a release from installing.
    pub fn check(&self) -> Result<()> {
        let problems = self.problems()?;
        if problems.is_empty() {
            return Ok(());
        }
        bail!(
            "the plugin isn't ready to release:\n{}",
            problems
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    fn problems(&self) -> Result<Vec<String>> {
        let manifest = &self.manifest;
        let plugin = &manifest.plugin;
        let mut problems = Vec::new();

        if !is_identifier(&plugin.handle) {
            problems.push(format!(
                "the handle `{}` isn't an identifier: letters, digits and `_`, not starting with a digit",
                plugin.handle
            ));
        }
        for (what, path) in [
            ("backend", &manifest.structure.backend),
            ("readme", &manifest.structure.readme),
            ("settings", &manifest.structure.settings),
            ("frontend path", &manifest.frontend.path),
        ] {
            if !stays_inside(path) {
                problems.push(format!(
                    "the {what} `{}` points outside the plugin",
                    path.display()
                ));
            }
        }
        if !self.dir.join(&manifest.structure.backend).is_file() {
            problems.push(format!(
                "the backend `{}` doesn't exist; build the plugin first",
                manifest.structure.backend.display()
            ));
        }
        for slot in &manifest.frontend.slots {
            let entry = manifest.frontend.path.join(&slot.entry);
            if !self.dir.join(&entry).is_file() {
                problems.push(format!(
                    "the `{}` slot's entry `{}` doesn't exist",
                    slot.slot,
                    entry.display()
                ));
            }
        }
        if let Some(cargo) = self.cargo()? {
            let version = cargo["package"]["version"].as_str();
            if version.is_some_and(|v| v != plugin.version.to_string()) {
                problems.push(format!(
                    "Cargo.toml is at version {} and {MANIFEST_FILE} at {}",
                    version.unwrap_or_default(),
                    plugin.version
                ));
            }
            let sdk = &cargo["dependencies"][SDK];
            let wanted = sdk.as_str().or_else(|| sdk["version"].as_str());
            let fits = wanted
                .and_then(|v| VersionReq::parse(v).ok())
                .is_none_or(|v| v.matches(&plugin.api));
            if !fits {
                problems.push(format!(
                    "{MANIFEST_FILE} says `api = \"{}\"`, and Cargo.toml builds against {SDK} {}; the two have to match",
                    plugin.api,
                    wanted.unwrap_or_default()
                ));
            }
        }
        Ok(problems)
    }

    /// The plugin's Cargo.toml, if it's written in Rust.
    pub fn cargo(&self) -> Result<Option<DocumentMut>> {
        let path = self.dir.join("Cargo.toml");
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(text.parse().context("Cargo.toml isn't valid")?))
    }

    /// What a release ships, as paths relative to the plugin, in order: the
    /// manifest and the files it points at.
    pub fn files(&self) -> Result<Vec<PathBuf>> {
        let manifest = &self.manifest;
        let mut files = vec![
            PathBuf::from(MANIFEST_FILE),
            manifest.structure.backend.clone(),
        ];
        for optional in [&manifest.structure.readme, &manifest.structure.settings] {
            if self.dir.join(optional).is_file() {
                files.push(optional.clone());
            }
        }
        if !manifest.frontend.slots.is_empty() {
            walk(&self.dir, &manifest.frontend.path, &mut files)?;
        }
        files.sort();
        files.dedup();
        Ok(files)
    }
}

fn walk(root: &Path, dir: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    let path = root.join(dir);
    for entry in
        std::fs::read_dir(&path).with_context(|| format!("couldn't read {}", path.display()))?
    {
        let entry = entry?;
        let relative = dir.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            walk(root, &relative, into)?;
        } else {
            into.push(relative);
        }
    }
    Ok(())
}

fn stays_inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// The server's rule for a handle.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
