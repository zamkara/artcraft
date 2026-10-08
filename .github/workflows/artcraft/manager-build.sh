#!/usr/bin/env bash
set -euo pipefail
job_dir=$1
pacman -Syu --noconfirm --needed rust python ca-certificates sudo
useradd --create-home --uid 1000 builder
chown -R builder:builder "$job_dir"
runuser -u builder -- bash -c 'cd "$1"; makepkg --cleanbuild --noconfirm; makepkg --printsrcinfo > .SRCINFO' bash "$job_dir"
cp "$job_dir/.cache/target/release/artcraft" "$job_dir/artcraft-linux-x86_64"
chmod 755 "$job_dir/artcraft-linux-x86_64"
python - "$job_dir" <<'PY'
import json,pathlib,subprocess,sys,hashlib
root=pathlib.Path(sys.argv[1]);packages=list(root.glob('*.pkg.tar.zst'))
if len(packages)!=1:raise SystemExit('Expected one native Artcraft package')
package=packages[0]
state=json.loads((root/'upstream.json').read_text());state['package']=package.name
(root/'upstream.json').write_text(json.dumps(state,indent=2)+'\n')
listing=subprocess.check_output(['bsdtar','-tf',str(package)],text=True).splitlines()
for member in ['.PKGINFO','.BUILDINFO','usr/bin/artcraft','usr/share/licenses/artcraft/LICENSE']:
 if member not in listing:raise SystemExit('Missing package member: '+member)
result=subprocess.run(['ldd',str(root/'artcraft-linux-x86_64')],capture_output=True,text=True)
if result.returncode or 'not found' in result.stdout:raise SystemExit('Unresolved Artcraft runtime dependencies')
with (root/'SHA256SUMS').open('w') as f:
 for path in [package,root/'artcraft-linux-x86_64',root/'PKGBUILD',root/'.SRCINFO',root/'upstream.json',root/'install.sh',root/'CHANGELOG.md']:
  digest=hashlib.sha256()
  with path.open('rb') as asset:
   for block in iter(lambda:asset.read(1024*1024),b''):digest.update(block)
  f.write(f'{digest.hexdigest()}  {path.name}\n')
print('Validated native Artcraft CLI package:',package.name)
PY
