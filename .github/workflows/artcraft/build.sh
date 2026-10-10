#!/usr/bin/env bash
# Runs inside the disposable Arch Linux container, never on the user's host.
set -euo pipefail
job_dir=$1
scripts_dir=$2
pacman -Syu --noconfirm --needed rust git pkgconf cmake clang nasm desktop-file-utils \
  libxkbcommon libxkbcommon-x11 wayland libx11 libxcb libxcursor libxi libxrandr \
  libglvnd vulkan-icd-loader xdg-desktop-portal alsa-lib python
if [[ "$(python -c 'import json,sys;print(json.load(open(sys.argv[1]))["kind"])' "$job_dir/upstream.json")" == tauri ]]; then
  pacman -S --noconfirm --needed nodejs npm webkit2gtk-4.1 gtk3 libsoup3 libappindicator-gtk3 openssl xdg-utils
fi
useradd --create-home --uid 1000 builder
chown -R builder:builder "$job_dir"
# Rust and native linkers may peak well beyond 7 GB without swap.
export MAKEFLAGS='-j2'
export ARTCRAFT_BUILD_JOBS=2
runuser -u builder -- bash -c 'cd "$1"; makepkg --cleanbuild --noconfirm; makepkg --printsrcinfo > .SRCINFO' bash "$job_dir"
python "$scripts_dir/validate.py" "$job_dir"
