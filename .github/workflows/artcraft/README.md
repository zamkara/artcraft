# Storytold native application suite

The workflow checks all registered upstream applications and Artcraft's own Rust source
at 20:17 UTC / 03:17 Asia/Jakarta every day. Manual dispatch and relevant pushes
also run the complete pipeline. Pull requests run validation.

All registered native Arch packages and the Artcraft manager are built independently in disposable Arch Linux
containers. Unchanged upstream commits and packaging inputs skip compilation.
Failed builds never advance the published checkpoints. Source snapshots, fonts,
and lockfiles come from pinned upstream revisions; no upstream repository or
vendored dependencies are committed here.

One final job publishes one complete suite release only after all build jobs
succeed. It includes Artcraft, every registered app package, the Artcraft binary,
installer, per-app PKGBUILD/SRCINFO/state/changelog files, suite.json, and shared
SHA256SUMS. Unchanged verified packages are carried forward from earlier releases.
The complete asset set is uploaded and checked as a draft before it becomes
public and latest. A partial suite is never published.

```sh
curl -fsSL https://github.com/zamkara/storytold/releases/latest/download/install.sh | bash
artcraft -ia
```

Artcraft discovers available packages from release assets. App names in apps.json
identify supported upstream projects; the list command only shows apps with
actual published packages. Pacman owns installations and upgrades. Both the piped
installer and pacman read confirmations from the terminal.

Full original upstream changelogs are separate APP.CHANGELOG.md assets. Release
notes link them and include original excerpts within GitHub's body-size limit.
Snapshots use upstream-version.rCOMMITTIMESTAMP.gCOMMIT and packaging changes
increment pkgrel. Builds, caches and source archives remain disposable runner
files, not repository content. The root contains LICENSE, .github, and the repository README.

See [CLI usage](cli/README.md).
