# Artcraft CLI

Artcraft manages native Storytold application packages for Arch Linux and Omarchy.
It installs packages with pacman, checks published release information,
verifies downloaded package checksums, and handles app and manager upgrades.

Install Artcraft:

```sh
curl -fsSL https://github.com/zamkara/storytold/releases/latest/download/install.sh | bash
```

The installer verifies the bootstrap binary and installs the native Artcraft
package through pacman. Pacman shows its normal transaction confirmation.
Only Linux x86_64 is supported by the current builds.

```sh
artcraft
artcraft -l
artcraft -s designcraft
artcraft -sn designcraft
artcraft -i designcraft photocraft
artcraft -ia
artcraft -S
artcraft -c
artcraft -o designcraft
artcraft -u
artcraft -u designcraft
artcraft -U
artcraft -r designcraft
```

`-u` without arguments upgrades installed applications and Artcraft itself.
It does not install apps you have not selected. `-ia` installs every supported upstream
application. Published releases must be available for every selected app;
missing releases are reported before installation begins.

Running `artcraft` in a terminal opens the integrated terminal interface.
Use `-t` to request it explicitly, or `-l` for the text catalog. The default
view contains only installed creative apps, with an empty state when none are
installed. Artcraft itself remains managed through `U` / `-U`, rather than
appearing as a creative app. Tables show installed and available versions.
The app catalog,
selected app details, changelogs, transaction confirmation, authentication,
and download / pacman progress remain inside the interface. No external
`dialog` program is required. Foreground and background use the terminal's
default colors; selected rows reverse them. There are no fixed RGB colors,
bold text, decorative icons, animations, or idle redraws.

Navigate with Up / Down, mark multiple apps with Space, and view details with
Enter. Use `i` to install, `u` to upgrade installed apps and Artcraft (or marked
apps), `U` to upgrade Artcraft only, `r` to remove, `o` to open, and `n` to view
the upstream changelog. `S` synchronizes the catalog and shows all published
applications; `c` checks the saved one. `a` shows the full saved catalog and
`l` returns to installed applications.
Tab focuses messages; PgUp / PgDn scrolls them. Enter on the messages pane
opens its complete contents. Esc closes details, or exits an idle interface.
Transactions show the resolved package plan and require explicit confirmation;
authentication input is masked. CLI commands retain pacman's normal interactive
confirmation. Errors stay visible and do not close the interface.

`-S` synchronizes the release catalog without changing installed packages.
`-c` compares installed versions against that saved catalog. Listing apps,
viewing details and opening the interface do not periodically synchronize.
Opening the interface and reading the saved catalog never triggers a network
request, including when no cache exists. Use `-S` or `-f` to fetch metadata.
`-u` refreshes the catalog and upgrades installed packages,
including Artcraft, in one operation; there is no need to run `-S` first.
Installation also refreshes the catalog. `-o APP` opens an installed application.
Details include package size; downloads show percentage and received bytes.
An optional `GH_TOKEN` or `GITHUB_TOKEN` increases GitHub's API request limit.
Tokens are sent only to GitHub's API, never to download hosts or cache files.

Artcraft verifies SHA-256, exact download size and the package's name/version
before invoking pacman. Checksums protect download integrity; releases are not
yet cryptographically signed. App files and installed versions remain managed
by pacman, including apps installed manually from this repository's packages.
No daemon, background service or separate installation database is created.

`-U` upgrades Artcraft itself; `-u` upgrades Artcraft and installed apps.
CLI pacman confirmation reads from `/dev/tty`, including piped installation.
The TUI shows its own transaction confirmation and masked authentication prompt.

The repository is `zamkara/storytold`; the command and manager package remain
`artcraft`. The upstream AI desktop app is packaged as `artcraft-studio` so it
can coexist with the manager. ArtCraft-X is packaged as `artcraftx`.
The upstream graphical launcher is excluded; Artcraft manages native Arch
packages directly.
After the repository rename, run the installation command above to update an
older manager whose release-URL checks predate the migration.

The manager contains no compiled application-name registry. Sync discovers
packages from release assets and reads the checksummed `suite.json` manifest
for descriptions and upstream links. Installed Storytold apps are identified
offline from pacman metadata. New apps appear after publication and sync
without rebuilding the manager. The workflow registry declares build targets.
