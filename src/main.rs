mod doctor;
mod new;
mod package;
mod project;
mod release;
mod run;

use anyhow::Result;
use clap::{Parser, Subcommand};
use semver::Version;

use crate::project::Project;

/// Create, build and release Lorealm plugins.
#[derive(Parser)]
#[command(name = "lorealm-plugin", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new plugin
    New(new::Args),
    /// Check that the tools for building and releasing plugins are installed
    Doctor {
        /// Install what's missing, where that's possible
        #[arg(long)]
        fix: bool,
    },
    /// Build the plugin in this folder
    Build,
    /// Build the plugin and check it's ready to release
    Check,
    /// Build the plugin and zip it into `dist`, the way a release ships it
    Package {
        /// Fail unless this is the tag of the plugin's version, such as `v0.1.0`
        #[arg(long)]
        tag: Option<String>,
    },
    /// Set the plugin's version, commit it and tag it, without pushing
    Stage {
        /// The version to release, such as `0.2.0`
        version: Version,
    },
    /// Push a staged release, which starts its release workflow
    Publish {
        /// The staged version. Defaults to the plugin's current one
        version: Option<Version>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New(args) => new::run(args),
        Command::Doctor { fix } => doctor::run(fix),
        Command::Build => Project::open(".")?.build(),
        Command::Check => {
            let project = Project::open(".")?;
            project.build()?;
            project.reopened()?.check()?;
            println!("Ready to release.");
            Ok(())
        }
        Command::Package { tag } => package::run(tag.as_deref()),
        Command::Stage { version } => release::stage(&version),
        Command::Publish { version } => release::publish(version.as_ref()),
    }
}
