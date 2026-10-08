"""Publish only after uploading and checking every asset in a draft release."""
import argparse,json,os,pathlib,subprocess
p=argparse.ArgumentParser();p.add_argument('directory');p.add_argument('--repository',required=True);p.add_argument('--target',required=True);args=p.parse_args()
root=pathlib.Path(args.directory);state=json.loads((root/'upstream.json').read_text())
tag=f"{state['app']}-{state['pkgver']}-{state['pkgrel']}"
assets=[root/state['package'],root/'PKGBUILD',root/'.SRCINFO',root/'upstream.json',root/'SHA256SUMS']
for path in assets:
 if not path.is_file():raise SystemExit(f'Missing release asset: {path}')
subprocess.run(['sha256sum','--check','SHA256SUMS'],cwd=root,check=True)
def gh(*cmd):return subprocess.check_output(['gh',*cmd],text=True)
try:release=json.loads(gh('release','view',tag,'--repo',args.repository,'--json','isDraft,assets,url'))
except subprocess.CalledProcessError:release=None
if release and not release['isDraft']:
 if {x.name for x in assets}<={x['name'] for x in release['assets']}:
  print('Already published:',release['url']);raise SystemExit(0)
 raise SystemExit('Refusing to modify an incomplete public release')
if not release:
 gh('release','create',tag,'--repo',args.repository,'--target',args.target,'--title',f"{state['app']} {state['pkgver']}-{state['pkgrel']} · Arch Linux x86_64",'--draft','--notes-file',str(root/'CHANGELOG.md'))
else:gh('release','edit',tag,'--repo',args.repository,'--notes-file',str(root/'CHANGELOG.md'))
gh('release','upload',tag,'--repo',args.repository,'--clobber',*[str(x) for x in assets])
release=json.loads(gh('release','view',tag,'--repo',args.repository,'--json','assets'))
actual={x['name']:x['size'] for x in release['assets']}
for path in assets:
 if actual.get(path.name)!=path.stat().st_size:raise SystemExit(f'Incomplete asset upload: {path.name}')
gh('release','edit',tag,'--repo',args.repository,'--draft=false','--latest=false')
print(gh('release','view',tag,'--repo',args.repository,'--json','url','--jq','.url').strip())
