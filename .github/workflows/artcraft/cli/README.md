# Artcraft CLI

Artcraft manages native creative-app packages for Arch Linux and Omarchy.
It installs packages with pacman, checks published release information,
verifies downloaded package checksums, and handles app and manager upgrades.

Install Artcraft:

```sh
curl -fsSL https://github.com/zamkara/artcraft/releases/latest/download/install.sh | bash
```

The installer verifies the bootstrap binary and installs the native Artcraft
package through pacman. Pacman shows its normal transaction confirmation.
Only Linux x86_64 is supported by the current builds.

```sh
artcraft -l
artcraft -s designcraft
artcraft -sn designcraft
artcraft -i designcraft photocraft
artcraft -ia
artcraft -c
artcraft -u
artcraft -u designcraft
artcraft -U
artcraft -r designcraft
```

`-u` without arguments updates installed creative apps and Artcraft itself.
It does not install apps you have not selected. `-ia` installs the seven
creative apps only. Published releases must be available for every selected app;
missing releases are reported before installation begins.

`-l` and `-s` cache release metadata for five minutes. `-f` forces a
fresh check. Installation, update checks and upgrades always check current
release metadata. An optional `GH_TOKEN` or `GITHUB_TOKEN` increases GitHub's
API request limit; no key is required for this public repository. Tokens are
sent only to GitHub's API, never to package download hosts or cache files.

Artcraft verifies SHA-256, exact download size and the package's name/version
before invoking pacman. Checksums protect download integrity; releases are not
yet cryptographically signed. App files and installed versions remain managed
by pacman, including apps installed manually from this repository's packages.
No daemon, background service or separate installation database is created.

`-U` upgrades Artcraft itself; `-u` upgrades Artcraft and installed apps.
Pacman confirmation always reads from `/dev/tty`, including piped installation.
