use std::path::Path;

use anyhow::{Context, Result, bail};
use lorealm_plugin_manifest::MANIFEST_FILE;
use semver::Version;
use toml_edit::{DocumentMut, value};

use crate::project::Project;
use crate::run::{output, run};

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    output(dir, "git", args)
}

/// Sets the plugin's version to `version`, commits that and tags it. Nothing
/// is pushed.
pub fn stage(version: &Version) -> Result<()> {
    let project = Project::open(".")?;
    let dir = &project.dir;
    let tag = format!("v{version}");

    git(dir, &["rev-parse", "--git-dir"])
        .context("this folder isn't a git repository; run `git init` and commit the plugin")?;
    let changes = git(dir, &["status", "--porcelain"])?;
    if !changes.is_empty() {
        bail!("commit or stash these first, so the release holds nothing by accident:\n{changes}");
    }
    if !git(dir, &["tag", "--list", &tag])?.is_empty() {
        bail!("the tag `{tag}` already exists");
    }
    let released = git(dir, &["tag", "--list", "v*"])?
        .lines()
        .filter_map(|t| Version::parse(t.trim_start_matches('v')).ok())
        .max();
    if let Some(released) = released.filter(|r| r >= version) {
        bail!("{version} isn't newer than {released}, which is already tagged");
    }
    let current = &project.manifest.plugin.version;
    if version < current {
        bail!("{version} is older than the plugin's version, {current}");
    }

    let result = write_version(&project, version).and_then(|()| {
        project.build()?;
        project.reopened()?.check()
    });
    if let Err(error) = result {
        // The tree was clean, so this only undoes the version it just wrote.
        git(dir, &["checkout", "--", "."])?;
        return Err(error);
    }

    if !git(dir, &["status", "--porcelain"])?.is_empty() {
        git(dir, &["add", "--update"])?;
        git(dir, &["commit", "--message", &format!("Release {tag}")])?;
    }
    git(dir, &["tag", "--annotate", &tag, "--message", &tag])?;
    println!("Staged {tag}. Release it with `lorealm-plugin publish`.");
    Ok(())
}

/// Writes `version` where the plugin keeps it.
fn write_version(project: &Project, version: &Version) -> Result<()> {
    let set = |file: &str, table: &str| -> Result<()> {
        let path = project.dir.join(file);
        if !path.is_file() {
            return Ok(());
        }
        let mut document: DocumentMut = std::fs::read_to_string(&path)?
            .parse()
            .with_context(|| format!("{file} isn't valid"))?;
        document[table]["version"] = value(version.to_string());
        std::fs::write(&path, document.to_string())?;
        Ok(())
    };
    set(MANIFEST_FILE, "plugin")?;
    set("Cargo.toml", "package")
}

/// Pushes the branch and the tag of a staged release.
pub fn publish(version: Option<&Version>) -> Result<()> {
    let project = Project::open(".")?;
    let dir = &project.dir;
    let version = version.unwrap_or(&project.manifest.plugin.version);
    let tag = format!("v{version}");

    let tagged = git(dir, &["rev-list", "--max-count=1", &tag]).map_err(|_| {
        anyhow::anyhow!("{version} isn't staged; run `lorealm-plugin stage {version}` first")
    })?;
    if tagged != git(dir, &["rev-parse", "HEAD"])? {
        bail!(
            "`{tag}` isn't the commit you're on; check out the tag's commit, or stage a new version"
        );
    }
    if !git(dir, &["status", "--porcelain"])?.is_empty() {
        bail!("there are changes since `{tag}` was staged; commit them and stage a new version");
    }
    if git(dir, &["remote", "get-url", "origin"]).is_err() {
        bail!(
            "there's nowhere to push to; add the plugin's repository with `git remote add origin <url>`"
        );
    }
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if branch == "HEAD" {
        bail!("you're not on a branch; check one out first");
    }

    run(dir, "git", &["push", "--atomic", "origin", &branch, &tag])?;
    println!(
        "Published {tag}. Its release appears once the repository's release workflow finishes."
    );
    Ok(())
}
