mod catalog;
mod tui;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::{Command as Process, ExitCode, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const REPOSITORY: &str = "zamkara/storytold";
const API: &str = "https://api.github.com/repos/zamkara/storytold";
const MAX_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Parser)]
#[command(
    version = option_env!("ARTCRAFT_PKGVER").unwrap_or(env!("CARGO_PKG_VERSION")),
    about = "Install and update native Storytold applications on Arch Linux",
    group(clap::ArgGroup::new("action").args(["list", "info", "install", "updates", "upgrade", "remove", "self_update", "bootstrap", "sync", "open", "tui"]))
)]
struct Cli {
    /// Open the terminal interface
    #[arg(short = 't', long)]
    tui: bool,
    /// Synchronize the release catalog without installing packages
    #[arg(short = 'S', long)]
    sync: bool,
    /// Open an installed app
    #[arg(short = 'o', long)]
    open: bool,
    /// List apps and available versions
    #[arg(short = 'l', long)]
    list: bool,
    /// Show app details
    #[arg(short = 's', long)]
    info: bool,
    /// Install selected apps
    #[arg(short = 'i', long)]
    install: bool,
    /// Show upgrades using the saved catalog; use -S to synchronize
    #[arg(short = 'c', long)]
    updates: bool,
    /// Upgrade installed apps and Artcraft, or selected apps
    #[arg(short = 'u', long)]
    upgrade: bool,
    /// Remove selected apps
    #[arg(short = 'r', long)]
    remove: bool,
    /// Upgrade Artcraft itself
    #[arg(short = 'U', long)]
    self_update: bool,
    /// Select all apps for installation or upgrade
    #[arg(short = 'a', long)]
    all: bool,
    /// Fetch fresh release metadata
    #[arg(short = 'f', long)]
    refresh: bool,
    /// Include changelog with app details
    #[arg(short = 'n', long, requires = "info")]
    changelog: bool,
    #[arg(short = 'B', hide = true, value_name = "RELEASE")]
    bootstrap: Option<String>,
    #[arg(value_name = "APP")]
    apps: Vec<String>,
}
enum Command {
    List,
    Tui,
    Sync,
    Open { app: String },
    Info { app: String, changelog: bool },
    Install { apps: Vec<String> },
    Updates,
    Upgrade { apps: Vec<String> },
    Remove { apps: Vec<String> },
    SelfUpdate,
    SelfInstall { release: String },
}
impl Cli {
    fn into_command(self) -> Result<Command> {
        ensure!(
            !self.all || self.install || self.upgrade,
            "Use -a with -i or -u"
        );
        ensure!(
            self.apps.is_empty()
                || self.install
                || self.upgrade
                || self.remove
                || self.info
                || self.open,
            "App names require -i, -u, -r, -s, or -o"
        );
        ensure!(
            !self.all || self.apps.is_empty(),
            "Use -a alone, or specify app names"
        );
        if self.open {
            ensure!(self.apps.len() == 1, "Use -o APP for one app");
            return Ok(Command::Open {
                app: self.apps[0].clone(),
            });
        }
        if self.tui {
            return Ok(Command::Tui);
        }
        if self.sync {
            return Ok(Command::Sync);
        }
        if self.install {
            let apps = if self.all {
                vec!["all".to_string()]
            } else {
                self.apps
            };
            ensure!(
                !apps.is_empty(),
                "Specify apps with -i APP, or install all with -ia"
            );
            return Ok(Command::Install { apps });
        }
        if self.upgrade {
            return Ok(Command::Upgrade { apps: self.apps });
        }
        if self.info {
            ensure!(self.apps.len() == 1, "Use -s APP for one app");
            return Ok(Command::Info {
                app: self.apps[0].clone(),
                changelog: self.changelog,
            });
        }
        if self.remove {
            ensure!(!self.apps.is_empty(), "Specify apps with -r APP");
            return Ok(Command::Remove { apps: self.apps });
        }
        if let Some(release) = self.bootstrap {
            return Ok(Command::SelfInstall { release });
        }
        if self.self_update {
            return Ok(Command::SelfUpdate);
        }
        if self.updates {
            return Ok(Command::Updates);
        }
        Ok(
            if !self.list && std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                Command::Tui
            } else {
                Command::List
            },
        )
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct App {
    repo: String,
    description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
struct Available {
    app: String,
    version: String,
    release: Release,
    package: Asset,
    info: App,
}
#[derive(Deserialize, Serialize)]
struct Cache {
    time: u64,
    releases: Vec<Release>,
    #[serde(default)]
    apps: BTreeMap<String, App>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")))?;
    Some(base.join("artcraft/releases.json"))
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .user_agent(concat!("artcraft/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?)
}
fn get(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut request = client.get(url);
    if url.starts_with("https://api.github.com/") {
        request = request
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(30));
        if let Ok(token) = std::env::var("GH_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")) {
            request = request.bearer_auth(token);
        }
    }
    let response = request
        .send()
        .with_context(|| format!("Could not fetch {url}"))?;
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        bail!(
            "GitHub request was rejected or rate-limited. Try later, or set GH_TOKEN for a higher API limit."
        );
    }
    let response = response.error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n <= limit),
        "Response exceeds the size limit"
    );
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "Response exceeds the size limit"
    );
    Ok(bytes)
}
fn asset_url<'a>(asset: &'a Asset, tag: &str) -> Result<&'a str> {
    let prefix = format!("https://github.com/{REPOSITORY}/releases/download/{tag}/");
    ensure!(
        asset.browser_download_url.starts_with(&prefix)
            || asset.browser_download_url.starts_with(&format!(
                "https://github.com/zamkara/artcraft/releases/download/{tag}/"
            )),
        "Release asset has an unexpected download URL"
    );
    ensure!(
        !asset.name.contains('/') && !asset.name.contains('\\') && !asset.name.starts_with('.'),
        "Invalid release asset name"
    );
    Ok(&asset.browser_download_url)
}
fn available(release: &Release, supported: &BTreeSet<String>) -> Option<Available> {
    if release.draft || release.prerelease {
        return None;
    }
    for app in supported {
        let prefix = format!("{app}-");
        let Some(package) = release
            .assets
            .iter()
            .find(|a| catalog::package_name(&a.name).is_some_and(|(name, _)| name == *app))
        else {
            continue;
        };
        if !release.assets.iter().any(|a| a.name == "SHA256SUMS") {
            return None;
        }
        let version = package
            .name
            .strip_prefix(&prefix)?
            .strip_suffix("-x86_64.pkg.tar.zst")?;
        if version.is_empty() || version.contains('/') {
            return None;
        }
        return Some(Available {
            app: app.clone(),
            version: version.to_string(),
            release: release.clone(),
            package: package.clone(),
            info: App {
                repo: String::new(),
                description: "Native Storytold application".into(),
            },
        });
    }
    None
}
fn installed(app: &str) -> Result<Option<String>> {
    let output = Process::new("/usr/bin/pacman")
        .env("LC_ALL", "C")
        .args(["-Q", app])
        .output()
        .context("pacman is required")?;
    if output.status.success() {
        return Ok(String::from_utf8(output.stdout)?
            .split_whitespace()
            .nth(1)
            .map(str::to_owned));
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("was not found") || error.contains("not found") {
        Ok(None)
    } else {
        bail!("Could not query pacman: {error}")
    }
}
fn newer(remote: &str, local: &str) -> Result<bool> {
    let output = Process::new("/usr/bin/vercmp")
        .args([remote, local])
        .output()
        .context("Arch vercmp is required")?;
    ensure!(output.status.success(), "vercmp failed");
    Ok(String::from_utf8(output.stdout)?.trim().parse::<i32>()? > 0)
}
fn select(
    requested: &[String],
    supported: &BTreeSet<String>,
    include_self: bool,
) -> Result<Vec<String>> {
    if requested.iter().any(|a| a == "all") {
        ensure!(
            requested.len() == 1,
            "Use 'all' alone, or list individual apps"
        );
        return Ok(supported
            .iter()
            .filter(|a| include_self || a.as_str() != "artcraft")
            .cloned()
            .collect());
    }
    let mut result = BTreeSet::new();
    for app in requested {
        ensure!(
            supported.contains(app) && (include_self || app != "artcraft"),
            "Unknown app '{app}'. Run 'artcraft -l' for available apps."
        );
        result.insert(app.clone());
    }
    Ok(result.into_iter().collect())
}
fn checksum(text: &str, filename: &str) -> Result<String> {
    let mut matches = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(hash), Some(name)) = (fields.next(), fields.next()) {
            if name.trim_start_matches('*') == filename {
                ensure!(
                    hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
                    "Invalid SHA-256 checksum"
                );
                matches.push(hash.to_ascii_lowercase());
            }
        }
    }
    ensure!(
        matches.len() == 1,
        "Missing or ambiguous checksum for {filename}"
    );
    Ok(matches.remove(0))
}
#[cfg(test)]
fn download(client: &Client, app: &Available, directory: &Path) -> Result<PathBuf> {
    download_with_feedback(client, app, directory, None)
}
fn download_with_feedback(
    client: &Client,
    app: &Available,
    directory: &Path,
    feedback: Option<&dyn Feedback>,
) -> Result<PathBuf> {
    let sums = app
        .release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .context("Release has no checksums")?;
    let sums = String::from_utf8(get(
        client,
        asset_url(sums, &app.release.tag_name)?,
        1024 * 1024,
    )?)?;
    let expected = checksum(&sums, &app.package.name)?;
    ensure!(
        app.package.size > 0 && app.package.size <= MAX_PACKAGE,
        "Package size is invalid"
    );
    let path = directory.join(&app.package.name);
    report(
        feedback,
        format!("Downloading {} {}...", app.app, app.version),
    );
    let mut response = client
        .get(asset_url(&app.package, &app.release.tag_name)?)
        .send()?
        .error_for_status()?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut digest = Sha256::new();
    let mut received = 0;
    let mut last_percent = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let length = response.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        received += length as u64;
        ensure!(
            received <= app.package.size,
            "Package exceeds the declared size"
        );
        digest.update(&buffer[..length]);
        file.write_all(&buffer[..length])?;
        let percent = received * 100 / app.package.size;
        if percent >= last_percent + 10 || received == app.package.size {
            report(
                feedback,
                format!(
                    "{}: {percent}% ({:.1} / {:.1} MiB)",
                    app.app,
                    received as f64 / 1048576.0,
                    app.package.size as f64 / 1048576.0
                ),
            );
            last_percent = percent;
        }
    }
    file.sync_all()?;
    ensure!(received == app.package.size, "Incomplete package download");
    ensure!(
        format!("{:x}", digest.finalize()) == expected,
        "Checksum verification failed for {}",
        app.app
    );
    let output = Process::new("/usr/bin/pacman")
        .env("LC_ALL", "C")
        .arg("-Qp")
        .arg(&path)
        .output()?;
    ensure!(
        output.status.success(),
        "Downloaded file is not a valid Arch package"
    );
    let identity = String::from_utf8(output.stdout)?;
    let mut fields = identity.split_whitespace();
    ensure!(
        fields.next() == Some(app.app.as_str()) && fields.next() == Some(app.version.as_str()),
        "Package identity does not match its release"
    );
    report(feedback, format!("Verified {} {}", app.app, app.version));
    Ok(path)
}
trait Feedback: Send + Sync {
    fn log(&self, text: String);
    fn confirm(&self, text: String) -> Result<bool>;
    fn password(&self) -> Result<Option<String>>;
}
fn report(feedback: Option<&dyn Feedback>, text: String) {
    if let Some(feedback) = feedback {
        feedback.log(text);
    } else {
        println!("{text}");
    }
}
fn pacman(arguments: &[String]) -> Result<()> {
    pacman_with_feedback(arguments, None)
}
fn pacman_with_feedback(arguments: &[String], feedback: Option<&dyn Feedback>) -> Result<()> {
    if let Some(feedback) = feedback {
        return tui::transaction(arguments, feedback);
    }
    ensure!(
        Path::new("/etc/arch-release").exists(),
        "Artcraft installation requires Arch Linux or an Arch derivative such as Omarchy"
    );
    let uid = Process::new("/usr/bin/id").arg("-u").output()?;
    let mut command = if String::from_utf8_lossy(&uid.stdout).trim() == "0" {
        Process::new("/usr/bin/pacman")
    } else {
        let mut command = Process::new("/usr/bin/sudo");
        command.args(["--", "/usr/bin/pacman"]);
        command
    };
    // Read confirmations from the terminal, never from piped installer source.
    let terminal = fs::File::open("/dev/tty")
        .context("An interactive terminal is required for pacman confirmation")?;
    command.stdin(Stdio::from(terminal));
    ensure!(
        command.args(arguments).status()?.success(),
        "pacman did not complete the transaction"
    );
    Ok(())
}
fn install(
    client: &Client,
    catalog: &BTreeMap<String, Available>,
    targets: &[String],
    upgrade_only: bool,
) -> Result<()> {
    install_with_feedback(client, catalog, targets, upgrade_only, None)
}
fn install_with_feedback(
    client: &Client,
    catalog: &BTreeMap<String, Available>,
    targets: &[String],
    upgrade_only: bool,
    feedback: Option<&dyn Feedback>,
) -> Result<()> {
    ensure!(
        std::env::consts::ARCH == "x86_64",
        "Only x86_64 packages are currently available"
    );
    let directory = tempfile::tempdir()?;
    let mut files = Vec::new();
    // Preflight all targets before downloading or modifying the system.
    let mut pending = Vec::new();
    for app in targets {
        let local = installed(app)?;
        if upgrade_only && local.is_none() {
            continue;
        }
        let remote = catalog.get(app).with_context(|| {
            format!(
                "No published Arch package for {app} yet. Check again after its build finishes."
            )
        })?;
        if let Some(local) = local {
            if !newer(&remote.version, &local)? {
                report(feedback, format!("{app} is up to date ({local})"));
                continue;
            }
        }
        pending.push(remote);
    }
    for remote in pending {
        files.push(download_with_feedback(
            client,
            remote,
            directory.path(),
            feedback,
        )?);
    }
    if files.is_empty() {
        report(feedback, "No packages to install.".into());
        return Ok(());
    }
    let mut arguments = vec!["-U".to_string(), "--".to_string()];
    arguments.extend(files.iter().map(|p| p.to_string_lossy().into_owned()));
    pacman_with_feedback(&arguments, feedback)
}

fn show_updates(supported: &BTreeSet<String>, catalog: &BTreeMap<String, Available>) -> Result<()> {
    let mut count = 0;
    for name in supported {
        if let Some(local) = installed(name)? {
            if let Some(remote) = catalog.get(name) {
                if newer(&remote.version, &local)? {
                    println!("{name}: {local} -> {}", remote.version);
                    count += 1;
                }
            } else {
                println!("{name}: no published release available");
            }
        }
    }
    if count == 0 {
        println!("No upgrades available in the saved catalog. Use -S to synchronize.");
    }
    Ok(())
}

fn main_result() -> Result<()> {
    let cli = Cli::parse();
    let local_apps = catalog::installed_apps()?;
    let local_names: BTreeSet<String> = local_apps
        .keys()
        .cloned()
        .chain(std::iter::once("artcraft".into()))
        .collect();
    let refresh = cli.refresh;
    let command = cli.into_command()?;
    if matches!(command, Command::Tui) {
        return tui::run(refresh);
    }
    if let Command::Open { app } = &command {
        ensure!(
            local_apps.contains_key(app),
            "Unknown installed app '{app}'"
        );
        ensure!(installed(app)?.is_some(), "{app} is not installed");
        Process::new(app)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Could not open {app}"))?;
        return Ok(());
    }
    if let Command::Remove { apps } = &command {
        let targets = select(apps, &local_names, true)?;
        let mut arguments = vec!["-R".to_string(), "--".to_string()];
        arguments.extend(targets);
        return pacman(&arguments);
    }
    let client = client()?;
    if let Command::SelfInstall { release } = &command {
        ensure!(
            release.starts_with("artcraft-")
                && release
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)),
            "Invalid Artcraft release tag"
        );
        let release: Release = serde_json::from_slice(&get(
            &client,
            &format!("{API}/releases/tags/{release}"),
            2 * 1024 * 1024,
        )?)?;
        let manager = available(&release, &BTreeSet::from(["artcraft".into()]))
            .context("Release has no native Artcraft package")?;
        let catalog = BTreeMap::from([("artcraft".to_string(), manager)]);
        return install(&client, &catalog, &["artcraft".to_string()], false);
    }
    let catalog = catalog::load(
        &client,
        refresh
            || matches!(
                command,
                Command::Upgrade { .. }
                    | Command::SelfUpdate
                    | Command::Sync
                    | Command::Install { .. }
            ),
    )?;
    let mut known: BTreeMap<String, App> = local_apps
        .iter()
        .map(|(name, (_, info))| (name.clone(), info.clone()))
        .collect();
    for (name, remote) in &catalog {
        known.insert(name.clone(), remote.info.clone());
    }
    let supported: BTreeSet<String> = known
        .keys()
        .cloned()
        .chain(std::iter::once("artcraft".into()))
        .collect();
    match command {
        Command::Sync => {
            println!("Release catalog synchronized. No packages were changed.");
            show_updates(&supported, &catalog)?;
        }
        Command::List => {
            if !known.keys().any(|name| catalog.contains_key(name)) {
                println!("The saved catalog is empty. Run artcraft -S to synchronize.");
                println!("Releases: https://github.com/zamkara/storytold/releases");
                return Ok(());
            }
            println!(
                "{:<20} {:<42} {:<42} STATUS",
                "APP", "INSTALLED", "AVAILABLE"
            );
            for name in known.keys().filter(|name| catalog.contains_key(*name)) {
                let local = installed(name)?.unwrap_or_else(|| "Not installed".to_string());
                let remote = catalog
                    .get(name)
                    .map(|a| a.version.as_str())
                    .unwrap_or("Not released yet");
                let status = if local == "Not installed" {
                    "Not installed"
                } else if newer(remote, &local)? {
                    "Upgrade available"
                } else {
                    "Current"
                };
                println!("{name:<20} {local:<42} {remote:<42} {status}");
            }
            println!(
                "\nInstall: artcraft -i APP\nSync catalog: artcraft -S\nCheck saved catalog: artcraft -c\nUpgrade: artcraft -u"
            );
        }
        Command::Info { app, changelog } => {
            ensure!(supported.contains(&app), "Unknown app '{app}'");
            println!("App: {app}");
            if let Some(info) = known.get(&app) {
                println!(
                    "Description: {}\nUpstream: https://github.com/{}",
                    info.description, info.repo
                );
            }
            println!(
                "Installed: {}",
                installed(&app)?.unwrap_or_else(|| "Not installed".to_string())
            );
            if let Some(remote) = catalog.get(&app) {
                println!(
                    "Available: {}\nPackage size: {:.2} MiB\nRelease: {}\nPublished: {}",
                    remote.version,
                    remote.package.size as f64 / 1048576.0,
                    remote.release.html_url,
                    remote.release.published_at.as_deref().unwrap_or("Unknown")
                );
                if changelog {
                    if let Some(asset) = remote.release.assets.iter().find(|a| {
                        a.name == format!("{app}.CHANGELOG.md") || a.name == "CHANGELOG.md"
                    }) {
                        println!(
                            "\n{}",
                            String::from_utf8(get(
                                &client,
                                asset_url(asset, &remote.release.tag_name)?,
                                4 * 1024 * 1024
                            )?)?
                        );
                    } else {
                        println!(
                            "\n{}",
                            remote
                                .release
                                .body
                                .as_deref()
                                .unwrap_or("No changelog available.")
                        );
                    }
                }
            } else {
                println!("Available: Not released yet");
            }
        }
        Command::Install { apps } => install(
            &client,
            &catalog,
            &select(&apps, &catalog.keys().cloned().collect(), false)?,
            false,
        )?,
        Command::Upgrade { apps } => {
            let targets = if apps.is_empty() {
                supported.iter().cloned().collect()
            } else {
                select(&apps, &supported, true)?
            };
            install(&client, &catalog, &targets, true)?;
        }
        Command::SelfUpdate => install(&client, &catalog, &["artcraft".to_string()], true)?,
        Command::Updates => show_updates(&supported, &catalog)?,
        Command::Remove { .. }
        | Command::SelfInstall { .. }
        | Command::Tui
        | Command::Open { .. } => unreachable!(),
    }
    Ok(())
}
fn main() -> ExitCode {
    match main_result() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release {
        Release { tag_name: "designcraft-1.0-1".into(), html_url: "https://github.com/zamkara/storytold/releases/tag/designcraft-1.0-1".into(), body: None, draft: false, prerelease: false, published_at: Some("2026-10-08T00:00:00Z".into()), assets: vec![Asset { name: "designcraft-1.0-1-x86_64.pkg.tar.zst".into(), browser_download_url: "https://github.com/zamkara/storytold/releases/download/designcraft-1.0-1/designcraft-1.0-1-x86_64.pkg.tar.zst".into(), size: 1 }, Asset { name: "SHA256SUMS".into(), browser_download_url: "https://github.com/zamkara/storytold/releases/download/designcraft-1.0-1/SHA256SUMS".into(), size: 70 }] }
    }

    #[test]
    fn short_flags_select_actions_and_reject_conflicts() {
        assert!(
            matches!(Cli::try_parse_from(["artcraft", "-ia"]).unwrap().into_command().unwrap(), Command::Install { apps } if apps == vec!["all"])
        );
        assert!(
            matches!(Cli::try_parse_from(["artcraft", "-u"]).unwrap().into_command().unwrap(), Command::Upgrade { apps } if apps.is_empty())
        );
        assert!(matches!(
            Cli::try_parse_from(["artcraft", "-U"])
                .unwrap()
                .into_command()
                .unwrap(),
            Command::SelfUpdate
        ));
        assert!(
            Cli::try_parse_from(["artcraft", "-i", "designcraft", "-r", "designcraft"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["artcraft", "-a"])
                .unwrap()
                .into_command()
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["artcraft", "-i"])
                .unwrap()
                .into_command()
                .is_err()
        );
        assert!(matches!(
            Cli::try_parse_from(["artcraft", "-sn", "designcraft"])
                .unwrap()
                .into_command()
                .unwrap(),
            Command::Info {
                changelog: true,
                ..
            }
        ));
    }
    #[test]
    fn sync_check_and_open_are_separate_actions() {
        assert!(matches!(
            Cli::try_parse_from(["artcraft", "-S"])
                .unwrap()
                .into_command()
                .unwrap(),
            Command::Sync
        ));
        assert!(matches!(
            Cli::try_parse_from(["artcraft", "-c"])
                .unwrap()
                .into_command()
                .unwrap(),
            Command::Updates
        ));
        assert!(
            matches!(Cli::try_parse_from(["artcraft", "-o", "designcraft"]).unwrap().into_command().unwrap(), Command::Open { app } if app == "designcraft")
        );
        assert!(Cli::try_parse_from(["artcraft", "-S", "-u"]).is_err());
        assert!(
            Cli::try_parse_from(["artcraft", "-o"])
                .unwrap()
                .into_command()
                .is_err()
        );
    }
    #[test]
    fn suite_release_exposes_each_published_package() {
        let mut release = release();
        release.tag_name = "artcraft-suite-20261008".into();
        release.assets.push(Asset { name: "photocraft-2.0-1-x86_64.pkg.tar.zst".into(), browser_download_url: "https://github.com/zamkara/storytold/releases/download/artcraft-suite-20261008/photocraft-2.0-1-x86_64.pkg.tar.zst".into(), size: 1 });
        for name in ["designcraft", "photocraft"] {
            assert!(available(&release, &BTreeSet::from([name.to_string()])).is_some());
        }
        assert!(available(&release, &BTreeSet::from(["filmcraft".to_string()])).is_none());
    }
    #[test]
    #[ignore = "Downloads a real published package; requires network and Arch pacman"]
    fn live_package_is_verified_without_installation() {
        let client = client().unwrap();
        let catalog = catalog::load(&client, true).unwrap();
        let remote = catalog.get("designcraft").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let package = download(&client, remote, directory.path()).unwrap();
        assert!(package.is_file());
        // No pacman transaction is invoked: this test only queries package metadata.
    }
    #[test]
    fn draft_and_incomplete_releases_cannot_be_installed() {
        let names = BTreeSet::from(["designcraft".to_string()]);
        let mut release = release();
        assert_eq!(available(&release, &names).unwrap().version, "1.0-1");
        release.draft = true;
        assert!(available(&release, &names).is_none());
        release.draft = false;
        release.assets.pop();
        assert!(available(&release, &names).is_none());
    }
    #[test]
    fn checksums_reject_missing_ambiguous_and_invalid_hashes() {
        let digest = "a".repeat(64);
        assert_eq!(
            checksum(&format!("{digest}  package\n"), "package").unwrap(),
            digest
        );
        assert!(checksum("abc package", "package").is_err());
        assert!(checksum(&format!("{digest} package\n{digest} package"), "package").is_err());
        assert!(checksum(&format!("{digest} other"), "package").is_err());
    }
    #[test]
    fn arbitrary_download_hosts_are_rejected() {
        let release = release();
        let mut asset = release.assets[0].clone();
        assert!(asset_url(&asset, &release.tag_name).is_ok());
        asset.browser_download_url = "https://example.com/package".into();
        assert!(asset_url(&asset, &release.tag_name).is_err());
    }
    #[test]
    fn selection_rejects_unknown_apps_and_mixed_all() {
        let supported = BTreeSet::from(["designcraft".to_string(), "artcraft".to_string()]);
        assert_eq!(
            select(&["all".into()], &supported, false).unwrap(),
            vec!["designcraft"]
        );
        assert!(select(&["all".into(), "designcraft".into()], &supported, false).is_err());
        assert!(select(&["unknown".into()], &supported, false).is_err());
    }
}
