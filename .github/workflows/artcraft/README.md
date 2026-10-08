# Arch Linux build workflow

Seven native x86_64 packages for Arch Linux and Omarchy. Each package contains the
upstream desktop app, CLI, desktop entry, icons, MIME types, metadata and licences.
Install a downloaded release package with `sudo pacman -U ./APP-VERSION-x86_64.pkg.tar.zst`.

The daily check runs at 20:17 UTC / 03:17 Asia/Jakarta. GitHub may delay scheduled
runs. `workflow_dispatch` can check all apps, one app, or only `artcraft`. Pushes build
and release the manager when its source changes. Pull requests run validation.
Creative-app builds run only on the daily schedule or manual dispatch, with up
to seven independent runners. The manager is published before app builds begin.

For each app, the upstream default-branch commit, the font commit pinned in its
release workflow, and the packaging recipe hash are compared with `upstream.json`
in the last successful published release for that app. Unchanged inputs skip all
build and release steps. A failed build never advances the release checkpoint.
Snapshots use `upstream-version.rCOMMITTIMESTAMP.gCOMMIT` so post-release commits
are explicitly distinguished from stable upstream tags. Recipe-only rebuilds
increment Arch `pkgrel`. Releases are independent for each app.

Source archives are pinned to commits and verified by SHA-256. Rust uses the
upstream lockfile and upstream Linux features, a generic x86_64 CPU target, and
the upstream embedded font input when present. Builds run as an unprivileged
user inside a disposable `archlinux:base-devel` container. No upstream source,
vendored dependencies, binary packages, build state or changelog is committed to
this repository. Runtime build files stay under `.github/workflows/.build/` on
the disposable runner; cached compilation files are stored by Actions.

Every published release includes the native package, its generated `PKGBUILD`,
`SRCINFO`, `upstream.json`, `CHANGELOG.md`, and `SHA256SUMS`.
`SRCINFO` contains makepkg's unmodified `.SRCINFO` content; its release filename
avoids GitHub's automatic renaming of leading-dot asset names. Package contents, missing shared
libraries, CLI startup and desktop entries are checked before publication. The
release is uploaded as a draft, verified, then made public. Re-running a failed
upload resumes the draft without rewriting an existing public release.

Full changelogs are downloadable without truncation. When notes exceed GitHub's
release-body limit, the body shows original upstream paragraphs and links to
the complete changelog asset.

Changelog text comes only from the upstream release body and upstream commit
messages. Initial builds include the most recent upstream release notes followed
by commits through the pinned build revision. Later builds include the upstream
commit messages between the two successfully published revisions.

These are source snapshots, not a fork or an assertion of Adobe compatibility.
Omarchy uses the same native Arch package format; no separate Omarchy binary
format or desktop configuration override is required. No host applications are
installed by this workflow. Signing and a pacman repository are outside the
initial build-and-release scope.

## Artcraft manager

The Rust CLI source lives in `cli/`. The manager release includes its native Arch
package, `artcraft-linux-x86_64`, and `install.sh`. Only manager releases are marked
as GitHub's latest release so the installation URL stays stable:

```sh
curl -fsSL https://github.com/zamkara/artcraft/releases/latest/download/install.sh | bash
```

See [CLI commands and verification](cli/README.md). App changelog assets retain
the complete upstream text; release bodies also include the manager installer.
