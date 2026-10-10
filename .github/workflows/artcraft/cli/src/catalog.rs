use super::*;

// The build registry is deliberately not embedded in the manager. The release
// assets and checksummed suite manifest are the published catalog authority.
pub(super) fn package_name(filename: &str) -> Option<(String, String)> {
    let stem = filename.strip_suffix("-x86_64.pkg.tar.zst")?;
    let (stem, rel) = stem.rsplit_once('-')?;
    let (name, version) = stem.rsplit_once('-')?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-+_.".contains(&b))
        || version.is_empty()
        || !rel.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        || rel.is_empty()
    {
        return None;
    }
    Some((name.into(), format!("{version}-{rel}")))
}
fn published(release: &Release) -> bool {
    !release.draft && !release.prerelease && release.assets.iter().any(|a| a.name == "SHA256SUMS")
}
fn selected_releases(mut releases: Vec<Release>) -> Vec<Release> {
    releases.retain(published);
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    if let Some(suite) = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name == "suite.json"))
    {
        return vec![suite.clone()];
    }
    releases
}
fn discover(releases: &[Release], info: &BTreeMap<String, App>) -> BTreeMap<String, Available> {
    let mut result = BTreeMap::new();
    for release in releases {
        for asset in &release.assets {
            let Some((name, _)) = package_name(&asset.name) else {
                continue;
            };
            if let Some(mut app) = available(release, &BTreeSet::from([name.clone()])) {
                if let Some(metadata) = info.get(&name) {
                    app.info = metadata.clone();
                }
                result.entry(name).or_insert(app);
            }
        }
    }
    result
}
fn manifest_info(client: &Client, release: &Release) -> Result<BTreeMap<String, App>> {
    let Some(asset) = release.assets.iter().find(|a| a.name == "suite.json") else {
        return Ok(BTreeMap::new());
    };
    let sums = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .context("No release checksums")?;
    let sums = String::from_utf8(get(
        client,
        asset_url(sums, &release.tag_name)?,
        1024 * 1024,
    )?)?;
    let data = get(
        client,
        asset_url(asset, &release.tag_name)?,
        4 * 1024 * 1024,
    )?;
    ensure!(
        format!("{:x}", Sha256::digest(&data)) == checksum(&sums, "suite.json")?,
        "Catalog manifest checksum mismatch"
    );
    let manifest: serde_json::Value = serde_json::from_slice(&data)?;
    let repo = manifest
        .get("repository")
        .and_then(|v| v.as_str())
        .context("Missing catalog repository")?;
    ensure!(
        repo == REPOSITORY || repo == "zamkara/artcraft",
        "Unexpected catalog repository"
    );
    let states = manifest
        .get("apps")
        .and_then(|v| v.as_object())
        .context("Invalid application catalog")?;
    let mut result = BTreeMap::new();
    for (name, state) in states {
        let filename = state
            .get("package")
            .and_then(|v| v.as_str())
            .context("Missing catalog package")?;
        ensure!(
            package_name(filename).is_some_and(|(pkg, _)| pkg == *name)
                && release.assets.iter().any(|a| a.name == filename),
            "Catalog package does not match release assets"
        );
        let repo = state.get("upstream").and_then(|v| v.as_str()).unwrap_or("");
        let description = state
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Native Storytold application");
        result.insert(
            name.clone(),
            App {
                repo: repo.into(),
                description: description.into(),
            },
        );
    }
    Ok(result)
}
pub(super) fn load(client: &Client, refresh: bool) -> Result<BTreeMap<String, Available>> {
    let path = cache_path();
    let cached = path
        .as_ref()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Cache>(&b).ok());
    if !refresh {
        return Ok(cached
            .map(|c| discover(&selected_releases(c.releases), &c.apps))
            .unwrap_or_default());
    }
    let mut releases = Vec::new();
    for page in 1..=50 {
        let values: Vec<Release> = serde_json::from_slice(&get(
            client,
            &format!("{API}/releases?per_page=100&page={page}"),
            16 * 1024 * 1024,
        )?)?;
        let end = values.len() < 100
            || values
                .iter()
                .any(|r| published(r) && r.assets.iter().any(|a| a.name == "suite.json"));
        releases.extend(values);
        if end {
            break;
        }
    }
    let selected = selected_releases(releases.clone());
    let mut info = BTreeMap::new();
    for release in &selected {
        info.extend(manifest_info(client, release)?);
    }
    let result = discover(&selected, &info);
    if let Some(path) = path {
        let parent = path.parent().context("Invalid cache path")?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(
            &mut file,
            &Cache {
                time: now(),
                releases,
                apps: info,
            },
        )?;
        file.persist(path)?;
    }
    Ok(result)
}
fn parse_installed(text: &str) -> BTreeMap<String, (String, App)> {
    let mut result = BTreeMap::new();
    for block in text.split("\n\n") {
        let fields: BTreeMap<&str, &str> = block
            .lines()
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect();
        let (Some(name), Some(version), Some(url)) =
            (fields.get("Name"), fields.get("Version"), fields.get("URL"))
        else {
            continue;
        };
        // Packages identify their real upstream in pacman's installed metadata;
        // this works offline, before the first catalog sync, and for future apps.
        let Some(repo) = url.strip_prefix("https://github.com/storytold/") else {
            continue;
        };
        if repo.is_empty() {
            continue;
        }
        result.insert(
            (*name).into(),
            (
                (*version).into(),
                App {
                    repo: format!("storytold/{repo}"),
                    description: fields
                        .get("Description")
                        .copied()
                        .unwrap_or("Native Storytold application")
                        .into(),
                },
            ),
        );
    }
    result
}
pub(super) fn installed_apps() -> Result<BTreeMap<String, (String, App)>> {
    let output = Process::new("/usr/bin/pacman")
        .env("LC_ALL", "C")
        .arg("-Qi")
        .output()
        .context("Pacman is required")?;
    ensure!(output.status.success(), "Could not read installed packages");
    Ok(parse_installed(&String::from_utf8(output.stdout)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn future_apps_are_discovered_without_a_compiled_registry() {
        let release = Release {
            tag_name: "artcraft-suite-test".into(),
            html_url: String::new(),
            body: None,
            draft: false,
            prerelease: false,
            published_at: Some("2026-10-11".into()),
            assets: vec![
                Asset {
                    name: "future-new-tool-2.5.r20261011-1-x86_64.pkg.tar.zst".into(),
                    browser_download_url: String::new(),
                    size: 1,
                },
                Asset {
                    name: "SHA256SUMS".into(),
                    browser_download_url: String::new(),
                    size: 1,
                },
            ],
        };
        let found = discover(&selected_releases(vec![release]), &BTreeMap::new());
        assert_eq!(found["future-new-tool"].version, "2.5.r20261011-1");
        assert_eq!(
            package_name("artcraft-studio-0.42.0-1-x86_64.pkg.tar.zst")
                .unwrap()
                .0,
            "artcraft-studio"
        );
        assert!(package_name("../bad-1-1-x86_64.pkg.tar.zst").is_none());
    }
    #[test]
    fn newest_suite_replaces_removed_apps_in_older_catalogs() {
        let mut old = Release {
            tag_name: "artcraft-suite-old".into(),
            html_url: String::new(),
            body: None,
            draft: false,
            prerelease: false,
            published_at: Some("2026-10-10".into()),
            assets: vec![
                Asset {
                    name: "removed-app-1.0-1-x86_64.pkg.tar.zst".into(),
                    browser_download_url: String::new(),
                    size: 1,
                },
                Asset {
                    name: "SHA256SUMS".into(),
                    browser_download_url: String::new(),
                    size: 1,
                },
                Asset {
                    name: "suite.json".into(),
                    browser_download_url: String::new(),
                    size: 1,
                },
            ],
        };
        let mut new = old.clone();
        new.published_at = Some("2026-10-11".into());
        new.assets[0].name = "future-tool-2.0-1-x86_64.pkg.tar.zst".into();
        let found = discover(&selected_releases(vec![old.clone(), new]), &BTreeMap::new());
        assert!(found.contains_key("future-tool"));
        assert!(!found.contains_key("removed-app"));
        old.draft = true;
        assert!(discover(&selected_releases(vec![old]), &BTreeMap::new()).is_empty());
    }
    #[test]
    fn installed_future_apps_are_detected_offline_from_package_metadata() {
        let found = parse_installed(
            "Name : future-tool\nVersion : 3.1-2\nURL : https://github.com/storytold/future\nDescription : A new app\n\nName : unrelated\nVersion : 1-1\nURL : https://example.com\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found["future-tool"].0, "3.1-2");
        assert_eq!(found["future-tool"].1.repo, "storytold/future");
        assert!(parse_installed("").is_empty());
    }
}
