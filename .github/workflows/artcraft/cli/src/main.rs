use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command as Process, ExitCode},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const REPOSITORY: &str = "zamkara/artcraft";
const API: &str = "https://api.github.com/repos/zamkara/artcraft";
const MAX_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Parser)]
#[command(
    version = option_env!("ARTCRAFT_PKGVER").unwrap_or(env!("CARGO_PKG_VERSION")),
    about = "Install and update native Artcraft creative apps on Arch Linux"
)]
struct Cli {
    /// Fetch current release information instead of using the five-minute cache
    #[arg(long, global = true)]
    refresh: bool,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// List creative apps and their installed and available versions
    List,
    /// Show application details and release information
    Info {
        app: String,
        #[arg(long)]
        changelog: bool,
    },
    /// Install one or more apps, or "all"
    Install {
        #[arg(required = true)]
        apps: Vec<String>,
    },
    /// Check installed apps and Artcraft for available updates
    Updates,
    /// Upgrade installed apps and Artcraft, or selected apps
    Upgrade { apps: Vec<String> },
    /// Uninstall selected apps through pacman
    Remove {
        #[arg(required = true)]
        apps: Vec<String>,
    },
    /// Upgrade Artcraft itself through pacman
    SelfUpdate,
    #[command(hide = true)]
    SelfInstall {
        #[arg(long)]
        release: String,
    },
}
#[derive(Deserialize)]
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
}
#[derive(Deserialize, Serialize)]
struct Cache {
    time: u64,
    releases: Vec<Release>,
}

fn apps() -> Result<BTreeMap<String, App>> {
    Ok(serde_json::from_str(include_str!("../../apps.json"))?)
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
        asset.browser_download_url.starts_with(&prefix),
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
        if !release.tag_name.starts_with(&prefix) {
            continue;
        }
        let package = release
            .assets
            .iter()
            .find(|a| a.name.starts_with(&prefix) && a.name.ends_with("-x86_64.pkg.tar.zst"))?;
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
        });
    }
    None
}
fn catalog(
    client: &Client,
    supported: &BTreeSet<String>,
    refresh: bool,
) -> Result<BTreeMap<String, Available>> {
    let path = cache_path();
    let cached = path
        .as_ref()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Cache>(&b).ok());
    let releases = if !refresh
        && cached
            .as_ref()
            .is_some_and(|c| now().saturating_sub(c.time) < 300)
    {
        cached.expect("cache was checked").releases
    } else {
        let mut collected = Vec::new();
        let mut found = BTreeSet::new();
        for page in 1..=50 {
            let values: Vec<Release> = serde_json::from_slice(&get(
                client,
                &format!("{API}/releases?per_page=100&page={page}"),
                16 * 1024 * 1024,
            )?)?;
            for release in &values {
                if let Some(a) = available(release, supported) {
                    found.insert(a.app);
                }
            }
            let end = values.len() < 100 || found.len() == supported.len();
            collected.extend(values);
            if end {
                break;
            }
        }
        if let Some(path) = path {
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_ok() {
                    if let Ok(mut file) = tempfile::NamedTempFile::new_in(parent) {
                        if serde_json::to_writer(
                            &mut file,
                            &Cache {
                                time: now(),
                                releases: collected.clone(),
                            },
                        )
                        .is_ok()
                        {
                            let _ = file.persist(path);
                        }
                    }
                }
            }
        }
        collected
    };
    // Choose the latest published release, not whichever draft was created first.
    let mut releases = releases;
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    let mut result = BTreeMap::new();
    for release in releases {
        if let Some(a) = available(&release, supported) {
            result.entry(a.app.clone()).or_insert(a);
        }
    }
    Ok(result)
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
            "Unknown app '{app}'. Run 'artcraft list' for available apps."
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
fn download(client: &Client, app: &Available, directory: &Path) -> Result<PathBuf> {
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
    println!("Downloading {} {}...", app.app, app.version);
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
    println!("Verified {} {}", app.app, app.version);
    Ok(path)
}
fn pacman(arguments: &[String]) -> Result<()> {
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
    // Let pacman display and confirm the transaction; never bypass its prompt.
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
                println!("{app} is up to date ({local})");
                continue;
            }
        }
        pending.push(remote);
    }
    for remote in pending {
        files.push(download(client, remote, directory.path())?);
    }
    if files.is_empty() {
        println!("No packages to install.");
        return Ok(());
    }
    let mut arguments = vec!["-U".to_string(), "--".to_string()];
    arguments.extend(files.iter().map(|p| p.to_string_lossy().into_owned()));
    pacman(&arguments)
}
fn main_result() -> Result<()> {
    let cli = Cli::parse();
    let known = apps()?;
    let mut supported: BTreeSet<String> = known.keys().cloned().collect();
    supported.insert("artcraft".to_string());
    let command = cli.command.unwrap_or(Command::List);
    if let Command::Remove { apps } = &command {
        let targets = select(apps, &supported, true)?;
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
        let manager =
            available(&release, &supported).context("Release has no native Artcraft package")?;
        let catalog = BTreeMap::from([("artcraft".to_string(), manager)]);
        return install(&client, &catalog, &["artcraft".to_string()], false);
    }
    let catalog = catalog(
        &client,
        &supported,
        cli.refresh
            || matches!(
                command,
                Command::Upgrade { .. }
                    | Command::SelfUpdate
                    | Command::Updates
                    | Command::Install { .. }
            ),
    )?;
    match command {
        Command::List => {
            println!("{:<15} {:<42} AVAILABLE", "APP", "INSTALLED");
            for name in known.keys() {
                let local = installed(name)?.unwrap_or_else(|| "Not installed".to_string());
                let remote = catalog
                    .get(name)
                    .map(|a| a.version.as_str())
                    .unwrap_or("Not released yet");
                println!("{name:<15} {local:<42} {remote}");
            }
            println!(
                "\nInstall: artcraft install <app>\nUpdates: artcraft updates\nUpgrade: artcraft upgrade"
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
                    "Available: {}\nRelease: {}\nPublished: {}",
                    remote.version,
                    remote.release.html_url,
                    remote.release.published_at.as_deref().unwrap_or("Unknown")
                );
                if changelog {
                    if let Some(asset) = remote
                        .release
                        .assets
                        .iter()
                        .find(|a| a.name == "CHANGELOG.md")
                    {
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
        Command::Install { apps } => {
            install(&client, &catalog, &select(&apps, &supported, false)?, false)?
        }
        Command::Upgrade { apps } => {
            let targets = if apps.is_empty() {
                supported.iter().cloned().collect()
            } else {
                select(&apps, &supported, true)?
            };
            install(&client, &catalog, &targets, true)?;
        }
        Command::SelfUpdate => install(&client, &catalog, &["artcraft".to_string()], true)?,
        Command::Updates => {
            let mut count = 0;
            for name in &supported {
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
                println!("No updates available for installed apps.");
            }
        }
        Command::Remove { .. } | Command::SelfInstall { .. } => unreachable!(),
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
        Release { tag_name: "designcraft-1.0-1".into(), html_url: "https://github.com/zamkara/artcraft/releases/tag/designcraft-1.0-1".into(), body: None, draft: false, prerelease: false, published_at: Some("2026-10-08T00:00:00Z".into()), assets: vec![Asset { name: "designcraft-1.0-1-x86_64.pkg.tar.zst".into(), browser_download_url: "https://github.com/zamkara/artcraft/releases/download/designcraft-1.0-1/designcraft-1.0-1-x86_64.pkg.tar.zst".into(), size: 1 }, Asset { name: "SHA256SUMS".into(), browser_download_url: "https://github.com/zamkara/artcraft/releases/download/designcraft-1.0-1/SHA256SUMS".into(), size: 70 }] }
    }

    #[test]
    #[ignore = "Downloads a real published package; requires network and Arch pacman"]
    fn live_package_is_verified_without_installation() {
        let client = client().unwrap();
        let supported = BTreeSet::from(["designcraft".to_string()]);
        let catalog = catalog(&client, &supported, true).unwrap();
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
