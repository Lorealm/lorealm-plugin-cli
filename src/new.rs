use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::ValueEnum;
use inquire::{Confirm, MultiSelect, Select, Text};

use crate::doctor;
use crate::project::{Project, SDK, is_identifier};
use crate::run::output;

/// The plugin API new plugins are written for, and the version of the SDK
/// that targets it.
const API: &str = "0.1.0";

#[derive(clap::Args)]
pub struct Args {
    /// Where to create it. Defaults to a folder named after its handle
    dir: Option<PathBuf>,
    /// Human-readable title
    #[arg(long)]
    name: Option<String>,
    /// Its unique identifier, such as `word_count`
    #[arg(long)]
    handle: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    author: Option<String>,
    /// A hook to handle. Repeat it for several
    #[arg(long = "hook", value_enum)]
    hooks: Vec<Hook>,
    /// Where it appears in the client
    #[arg(long, value_enum)]
    slot: Option<Slot>,
    /// What the slot is written in. `typescript` builds it with Vite and
    /// needs Node, and in return IDEs complete the `lorealm` API
    #[arg(long, value_enum)]
    frontend: Option<Frontend>,
    /// Give it settings the user can fill in
    #[arg(long)]
    settings: bool,
    /// The license to put in LICENSE. `none` writes no LICENSE.
    #[arg(long, value_enum)]
    license: Option<License>,
    /// Don't ask anything; use the options given, and defaults for the rest
    #[arg(long, short)]
    yes: bool,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Slot {
    Inspector,
    Message,
    Composer,
}

impl Slot {
    fn name(self) -> &'static str {
        match self {
            Self::Inspector => "inspector",
            Self::Message => "message",
            Self::Composer => "composer",
        }
    }

    /// Its context's type in `@lorealm/plugin`.
    fn context(self) -> &'static str {
        match self {
            Self::Inspector => "InspectorContext",
            Self::Message => "MessageContext",
            Self::Composer => "ComposerContext",
        }
    }
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Frontend {
    /// One HTML file with its script inline, built by nothing
    Html,
    /// TypeScript on `@lorealm/plugin`, bundled into one HTML file by Vite
    Typescript,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum License {
    Mit,
    #[value(name = "apache-2.0")]
    Apache2,
    #[value(name = "gpl-3.0")]
    Gpl3,
    #[value(name = "mpl-2.0")]
    Mpl2,
    Unlicense,
    None,
}

impl License {
    /// Its SPDX identifier, for `license` in Cargo.toml.
    fn spdx(self) -> Option<&'static str> {
        match self {
            Self::Mit => Some("MIT"),
            Self::Apache2 => Some("Apache-2.0"),
            Self::Gpl3 => Some("GPL-3.0-only"),
            Self::Mpl2 => Some("MPL-2.0"),
            Self::Unlicense => Some("Unlicense"),
            Self::None => None,
        }
    }

    fn about(self) -> &'static str {
        match self {
            Self::Mit => "MIT: do anything, keep the notice",
            Self::Apache2 => "Apache-2.0: like MIT, with a patent grant",
            Self::Gpl3 => "GPL-3.0: changed versions must stay open source",
            Self::Mpl2 => "MPL-2.0: changed files must stay open source",
            Self::Unlicense => "Unlicense: public domain",
            Self::None => "None: I'll add my own",
        }
    }

    /// The text of LICENSE, with `{{year}}` and `{{author}}` for the
    /// copyright line where the license has one.
    fn template(self) -> Option<&'static str> {
        match self {
            Self::Mit => Some(include_str!("../templates/licenses/MIT")),
            Self::Apache2 => Some(include_str!("../templates/licenses/Apache-2.0")),
            Self::Gpl3 => Some(include_str!("../templates/licenses/GPL-3.0")),
            Self::Mpl2 => Some(include_str!("../templates/licenses/MPL-2.0")),
            Self::Unlicense => Some(include_str!("../templates/licenses/Unlicense")),
            Self::None => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Hook {
    UserMessage,
    PromptBuild,
    LorebookActivate,
    MessageFinalize,
    AfterGenerate,
    ToolCall,
    ToolResult,
    TurnStart,
    TurnEnd,
    Upgrade,
    SettingsChanged,
    Shutdown,
}

impl Hook {
    fn name(self) -> String {
        self.to_possible_value()
            .expect("no hook is skipped")
            .get_name()
            .to_owned()
    }

    /// `Hook::{variant}` in the SDK.
    fn variant(self) -> String {
        self.name()
            .split('-')
            .map(|word| word[..1].to_uppercase() + &word[1..])
            .collect()
    }

    fn about(self) -> &'static str {
        match self {
            Self::UserMessage => "change or refuse what the user sends",
            Self::PromptBuild => "change the prompt before it's sent to the model",
            Self::LorebookActivate => "change which lorebook entries are inserted",
            Self::MessageFinalize => "change a reply before it's saved",
            Self::AfterGenerate => "react to a saved reply",
            Self::ToolCall => "change or refuse the model's tool calls",
            Self::ToolResult => "change what a tool returned",
            Self::TurnStart => "react to a turn starting",
            Self::TurnEnd => "react to a finished turn",
            Self::Upgrade => "migrate data from another version of the plugin",
            Self::SettingsChanged => "react to the user saving settings",
            Self::Shutdown => "save things before the plugin is unloaded",
        }
    }

    /// The permission the server asks for before it registers the hook.
    fn permission(self) -> Option<&'static str> {
        match self {
            Self::PromptBuild | Self::LorebookActivate => Some("prompt:write"),
            Self::AfterGenerate | Self::TurnStart | Self::TurnEnd => Some("chat:read"),
            Self::MessageFinalize | Self::UserMessage => Some("chat:write"),
            Self::ToolCall | Self::ToolResult => Some("tools:write"),
            Self::Upgrade | Self::SettingsChanged | Self::Shutdown => None,
        }
    }

    /// Its method in `impl Plugin`, doing nothing, and the SDK types that
    /// names.
    fn stub(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::UserMessage => (
                "fn user_message(&mut self, _input: UserInput) -> Result<Verdict> {\n        Ok(Verdict::Allow)\n    }",
                &["UserInput", "Verdict"],
            ),
            Self::PromptBuild => (
                "fn prompt_build(&mut self, prompt: PromptAssembly) -> Result<Vec<Message>> {\n        Ok(prompt.messages)\n    }",
                &["Message", "PromptAssembly"],
            ),
            Self::LorebookActivate => (
                "fn lorebook_activate(&mut self, activation: LorebookActivation) -> Result<Vec<LorebookEntry>> {\n        Ok(activation.entries)\n    }",
                &["LorebookActivation", "LorebookEntry"],
            ),
            Self::MessageFinalize => (
                "fn message_finalize(&mut self, reply: Finalizing) -> Result<String> {\n        Ok(reply.content)\n    }",
                &["Finalizing"],
            ),
            Self::AfterGenerate => (
                "fn after_generate(&mut self, _reply: Generated) -> Result<()> {\n        Ok(())\n    }",
                &["Generated"],
            ),
            Self::ToolCall => (
                "fn tool_call(&mut self, _call: ToolInvocation) -> Result<Verdict> {\n        Ok(Verdict::Allow)\n    }",
                &["ToolInvocation", "Verdict"],
            ),
            Self::ToolResult => (
                "fn tool_result(&mut self, output: ToolOutput) -> Result<Value> {\n        Ok(serde_json::from_str(&output.result)?)\n    }",
                &["ToolOutput", "Value", "serde_json"],
            ),
            Self::TurnStart => (
                "fn turn_start(&mut self, _turn: TurnStarted) -> Result<()> {\n        Ok(())\n    }",
                &["TurnStarted"],
            ),
            Self::TurnEnd => (
                "fn turn_end(&mut self, _turn: TurnEnded) -> Result<()> {\n        Ok(())\n    }",
                &["TurnEnded"],
            ),
            Self::Upgrade => (
                "fn upgrade(&mut self, _from: Option<String>) -> Result<()> {\n        Ok(())\n    }",
                &[],
            ),
            Self::SettingsChanged => (
                "fn settings_changed(&mut self) -> Result<()> {\n        Ok(())\n    }",
                &[],
            ),
            Self::Shutdown => (
                "fn shutdown(&mut self) -> Result<()> {\n        Ok(())\n    }",
                &[],
            ),
        }
    }
}

struct Answers {
    name: String,
    handle: String,
    description: String,
    author: String,
    hooks: Vec<Hook>,
    slot: Option<Slot>,
    frontend: Frontend,
    settings: bool,
    license: License,
}

impl Answers {
    fn typescript(&self) -> bool {
        self.slot.is_some() && self.frontend == Frontend::Typescript
    }
}

pub fn run(args: Args) -> Result<()> {
    let dir = args.dir.clone();
    let answers = ask(args)?;
    let dir = dir.unwrap_or_else(|| PathBuf::from(&answers.handle));
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        bail!("{} already has something in it", dir.display());
    }
    write(&dir, &answers)?;
    let _ = output(&dir, "cargo", &["fmt"]);

    Project::open(&dir)?;

    if output(&dir, "git", &["rev-parse", "--git-dir"]).is_err() {
        let _ = output(&dir, "git", &["init"]);
    }

    // Installing writes the lockfile the release workflow installs from.
    let installed = answers.typescript()
        && output(
            &dir.join("frontend"),
            "npm",
            &["install", "--no-audit", "--no-fund"],
        )
        .is_ok();

    println!("Created {} in {}", answers.name, dir.display());
    if answers.typescript() && !installed {
        println!(
            "Install the frontend's dependencies with `npm install` in its `frontend` folder."
        );
    }
    println!("Build it with `lorealm-plugin build` within the folder.");

    let (missing, _) = doctor::missing(false, answers.typescript())?;
    for line in missing {
        println!("{line}");
    }
    Ok(())
}

fn ask(args: Args) -> Result<Answers> {
    let quiet = args.yes;
    let text = |given: Option<String>, question: &str, default: String| -> Result<String> {
        match given {
            Some(given) => Ok(given),
            None if quiet => Ok(default),
            None => Ok(Text::new(question).with_default(&default).prompt()?),
        }
    };

    let name = text(args.name, "Name", "My Plugin".into())?;
    let handle = text(args.handle, "Handle (its unique identifier)", slug(&name))?;
    if !is_identifier(&handle) {
        bail!(
            "the handle `{handle}` isn't an identifier: letters, digits and `_`, not starting with a digit"
        );
    }
    let description = text(
        args.description,
        "Description, in a sentence",
        String::new(),
    )?;
    let git_name = output(Path::new("."), "git", &["config", "user.name"]).unwrap_or_default();
    let author = text(args.author, "Author", git_name)?;

    let mut hooks = args.hooks;
    let mut slot = args.slot;
    let mut frontend = args.frontend;
    let mut settings = args.settings;
    if !quiet {
        if hooks.is_empty() {
            let labels: Vec<String> = Hook::value_variants()
                .iter()
                .map(|hook| format!("{}: {}", hook.name(), hook.about()))
                .collect();
            let picked = MultiSelect::new("Hooks to handle", labels.clone()).prompt()?;
            hooks = Hook::value_variants()
                .iter()
                .zip(&labels)
                .filter(|(_, label)| picked.contains(label))
                .map(|(hook, _)| *hook)
                .collect();
        }
        if slot.is_none() {
            let places = vec![
                "nowhere",
                "inspector: a widget in the inspector",
                "message: under replies",
                "composer: above the chat bar",
            ];
            let picked = Select::new("Where it appears in the client", places.clone()).prompt()?;
            slot = [Slot::Inspector, Slot::Message, Slot::Composer]
                .into_iter()
                .find(|slot| picked.starts_with(slot.name()));
        }
        if slot.is_some() && frontend.is_none() {
            let kinds = vec![
                "HTML: one file, nothing to build",
                "TypeScript: IDEs complete the API; built with Vite, needs Node",
            ];
            let picked = Select::new("Write the frontend in", kinds).raw_prompt()?;
            frontend = Some(Frontend::value_variants()[picked.index]);
        }
        if !settings {
            settings = Confirm::new("Give it settings the user can fill in?")
                .with_default(false)
                .prompt()?;
        }
    }
    // Without being asked, it writes no license: choosing one is the
    // author's call.
    let license = match args.license {
        Some(license) => license,
        None if quiet => License::None,
        None => {
            let labels: Vec<&str> = License::value_variants()
                .iter()
                .map(|license| license.about())
                .collect();
            let picked = Select::new("License", labels).raw_prompt()?;
            License::value_variants()[picked.index]
        }
    };
    hooks.dedup();

    Ok(Answers {
        name,
        handle,
        description,
        author,
        hooks,
        slot,
        frontend: frontend.unwrap_or(Frontend::Html),
        settings,
        license,
    })
}

/// `name` as a handle: `My Plugin!` is `my_plugin`.
fn slug(name: &str) -> String {
    let slug = name
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    match slug.chars().next() {
        Some(first) if !first.is_ascii_digit() => slug,
        _ => format!("plugin_{slug}").trim_end_matches('_').to_owned(),
    }
}

fn write(dir: &Path, answers: &Answers) -> Result<()> {
    let put = |path: &str, content: String| -> Result<()> {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().expect("a file is in a folder"))?;
        Ok(std::fs::write(path, content)?)
    };
    put("manifest.toml", manifest(answers))?;
    put("Cargo.toml", cargo(answers))?;
    put("src/lib.rs", source(answers))?;
    put(
        ".gitignore",
        if answers.typescript() {
            "target\ndist\nnode_modules\n"
        } else {
            "target\ndist\n"
        }
        .into(),
    )?;
    put(
        "README.md",
        include_str!("../templates/README.md")
            .replace("{{name}}", &answers.name)
            .replace("{{description}}", &answers.description),
    )?;
    // ubuntu-latest comes with Node, so the frontend only needs installing.
    let npm_step = if answers.typescript() {
        "      - run: npm ci --prefix frontend\n"
    } else {
        ""
    };
    put(
        ".github/workflows/release.yml",
        include_str!("../templates/release.yml").replace("{{npm}}\n", npm_step),
    )?;
    match answers.slot {
        Some(slot) if answers.typescript() => {
            let fill = |template: &str| {
                template
                    .replace("{{slot}}", slot.name())
                    .replace("{{context}}", slot.context())
                    .replace("{{api}}", API)
            };
            for (path, template) in [
                (
                    "package.json",
                    include_str!("../templates/typescript/package.json"),
                ),
                (
                    "tsconfig.json",
                    include_str!("../templates/typescript/tsconfig.json"),
                ),
                (
                    "vite.config.ts",
                    include_str!("../templates/typescript/vite.config.ts"),
                ),
                (
                    &format!("{}.html", slot.name()),
                    include_str!("../templates/typescript/slot.html"),
                ),
                (
                    &format!("src/{}.ts", slot.name()),
                    include_str!("../templates/typescript/slot.ts"),
                ),
            ] {
                put(&format!("frontend/{path}"), fill(template))?;
            }
        }
        Some(slot) => put(
            &format!("frontend/{}.html", slot.name()),
            include_str!("../templates/slot.html").into(),
        )?,
        None => {}
    }
    if answers.settings {
        put(
            "settings.wtn",
            "#inputs\ngreeting: string = \"Hello\"\n".into(),
        )?;
    }
    if let Some(template) = answers.license.template() {
        let holder = if answers.author.is_empty() {
            format!("the {} authors", answers.name)
        } else {
            answers.author.clone()
        };
        put(
            "LICENSE",
            template
                .replace("{{year}}", &this_year().to_string())
                .replace("{{author}}", &holder),
        )?;
    }
    Ok(())
}

/// The current year, in UTC.
fn this_year() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    year_of(seconds.div_euclid(86_400))
}

/// The year of the day `days` after 1970-01-01, from Howard Hinnant's
/// `civil_from_days`.
fn year_of(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(month <= 2)
}

/// A TOML string.
fn quoted(text: &str) -> String {
    toml_edit::Value::from(text).to_string()
}

fn manifest(answers: &Answers) -> String {
    let mut permissions: Vec<&str> = answers
        .hooks
        .iter()
        .filter_map(|h| h.permission())
        .collect();
    permissions.sort();
    permissions.dedup();
    let permissions: Vec<String> = permissions.into_iter().map(quoted).collect();

    let mut manifest = format!(
        r#"[plugin]
name = {name}
handle = {handle}
description = {description}
tags = []
author = {author}
version = "0.1.0"
# The plugin API this is built for, which is the version of `{SDK}` in
# Cargo.toml. A server only runs plugins built for its own.
api = "{API}"
permissions = [{permissions}]

[structure]
backend = "target/wasm32-wasip2/release/{crate_name}.wasm"

[build]
backend = ["cargo", "build", "--release", "--target", "wasm32-wasip2"]
{frontend_build}"#,
        name = quoted(&answers.name),
        handle = quoted(&answers.handle),
        description = quoted(&answers.description),
        author = quoted(&answers.author),
        permissions = permissions.join(", "),
        crate_name = answers.handle,
        frontend_build = if answers.typescript() {
            "frontend = [\"npm\", \"run\", \"build\", \"--prefix\", \"frontend\"]\n"
        } else {
            ""
        },
    );
    if answers.typescript() {
        // Vite builds the slot into one HTML file here.
        manifest.push_str("\n[frontend]\npath = \"frontend/dist\"\n");
    }
    if let Some(slot) = answers.slot {
        manifest.push_str(&format!(
            "\n[[frontend.slots]]\nslot = \"{slot}\"\nentry = \"{slot}.html\"\ntitle = {title}\n",
            slot = slot.name(),
            title = quoted(&answers.name),
        ));
    }
    manifest
}

fn cargo(answers: &Answers) -> String {
    let license = answers
        .license
        .spdx()
        .map(|spdx| format!("license = \"{spdx}\"\n"))
        .unwrap_or_default();
    format!(
        r#"[package]
name = "{handle}"
version = "0.1.0"
edition = "2024"
{license}publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
{SDK} = "{API}"
"#,
        handle = answers.handle
    )
}

fn source(answers: &Answers) -> String {
    let mut imports = vec!["Plugin", "Result"];
    let mut methods = Vec::new();
    let mut registers = Vec::new();
    if !answers.hooks.is_empty() {
        imports.extend(["Hook", "host"]);
    }
    for hook in &answers.hooks {
        let (method, types) = hook.stub();
        imports.extend(types);
        methods.push(method.to_owned());
        registers.push(format!(
            "        host::register_hook(Hook::{})?;\n",
            hook.variant()
        ));
    }
    if answers.slot.is_some() {
        imports.extend(["FrontendCall", "Value", "json"]);
        methods.push(
            "/// A call from the plugin's frontend, made with `lorealm.call(name, args)`.\n    fn frontend(&mut self, call: FrontendCall) -> Result<Value> {\n        match call.name.as_str() {\n            \"hello\" => Ok(json!(\"Hello from the backend!\")),\n            other => Err(format!(\"unknown call `{other}`\").into()),\n        }\n    }"
                .to_owned(),
        );
    }
    imports.sort();
    imports.dedup();

    let type_name: String = answers
        .handle
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| word[..1].to_uppercase() + &word[1..])
        .collect();
    let methods: String = methods
        .iter()
        .map(|method| format!("\n    {method}\n"))
        .collect();
    format!(
        r#"use lorealm_plugin::{{{imports}}};

/// Lives for as long as the plugin is loaded. Keep what it remembers between
/// calls in here.
struct {type_name};

impl Plugin for {type_name} {{
    /// Called once when the plugin is loaded.
    fn init() -> Result<Self> {{
{registers}        Ok(Self)
    }}
{methods}}}

lorealm_plugin::export!({type_name});
"#,
        imports = imports.join(", "),
        registers = registers.concat(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_handle() {
        assert_eq!(slug("My Plugin!"), "my_plugin");
        assert_eq!(slug("3D  Dice"), "plugin_3d_dice");
        assert_eq!(slug("???"), "plugin");
    }

    #[test]
    fn hooks_are_named_as_the_sdk_names_them() {
        assert_eq!(Hook::AfterGenerate.name(), "after-generate");
        assert_eq!(Hook::AfterGenerate.variant(), "AfterGenerate");
    }

    #[test]
    fn days_become_years() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(-1), 1969);
        assert_eq!(year_of(10_957), 2000); // 2000-01-01
        assert_eq!(year_of(11_322), 2000); // 2000-12-31
        assert_eq!(year_of(20_770), 2026); // 2026-11-13
    }

    #[test]
    fn every_license_but_none_has_a_text_and_an_identifier() {
        for license in License::value_variants() {
            assert_eq!(license.template().is_some(), license.spdx().is_some());
        }
        assert!(License::Mit.template().unwrap().contains("{{author}}"));
    }
}
