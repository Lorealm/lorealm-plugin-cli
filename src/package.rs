use std::io::Write;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::project::Project;

/// Builds the plugin and zips what a release ships into `dist`.
pub fn run(tag: Option<&str>) -> Result<()> {
    let project = Project::open(".")?;
    let plugin = &project.manifest.plugin;
    if let Some(tag) = tag
        && tag != format!("v{}", plugin.version)
    {
        bail!(
            "the tag is `{tag}`, and the plugin is at version {}; its tag is `v{}`",
            plugin.version,
            plugin.version
        );
    }
    project.build()?;
    let project = project.reopened()?;
    project.check()?;

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for file in project.files()? {
        let bytes = std::fs::read(project.dir.join(&file))
            .with_context(|| format!("couldn't read {}", file.display()))?;
        // Zip paths use `/` everywhere.
        let name = file
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(&bytes)?;
    }
    let bytes = zip.finish()?.into_inner();

    let plugin = &project.manifest.plugin;
    let dist = project.dir.join("dist");
    std::fs::create_dir_all(&dist)?;
    let path = dist.join(format!("{}-{}.zip", plugin.handle, plugin.version));
    std::fs::write(&path, &bytes)?;

    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!("Packaged {}", path.display());
    println!("sha256 {digest}");
    Ok(())
}
