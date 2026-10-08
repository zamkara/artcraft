"""Publish only after uploading and checking every asset in a draft release."""
import argparse,hashlib,json,pathlib,subprocess,tempfile

def release_notes(text,repository,tag):
    if len(text)<=115000:return text
    excerpt=text[:115000].rsplit('\n\n',1)[0]
    return excerpt+f'\n\n[Complete upstream changelog](https://github.com/{repository}/releases/download/{tag}/CHANGELOG.md)\n'

def export_assets(root,directory,state):
    # GitHub renames leading-dot asset names; retain the content as SRCINFO.
    srcinfo=directory/'SRCINFO';srcinfo.write_bytes((root/'.SRCINFO').read_bytes())
    assets=[root/state['package'],root/'PKGBUILD',srcinfo,root/'upstream.json',root/'CHANGELOG.md']
    checksums=directory/'SHA256SUMS'
    with checksums.open('w') as stream:
        for path in assets:
            digest=hashlib.sha256()
            with path.open('rb') as f:
                for block in iter(lambda:f.read(1024*1024),b''):digest.update(block)
            stream.write(f'{digest.hexdigest()}  {path.name}\n')
    return assets+[checksums]

def publish(root,repository,target,directory):
    state=json.loads((root/'upstream.json').read_text())
    tag=f"{state['app']}-{state['pkgver']}-{state['pkgrel']}"
    for path in [root/state['package'],root/'PKGBUILD',root/'.SRCINFO',root/'upstream.json',root/'SHA256SUMS',root/'CHANGELOG.md']:
        if not path.is_file():raise SystemExit(f'Missing release asset: {path}')
    subprocess.run(['sha256sum','--check','SHA256SUMS'],cwd=root,check=True)
    assets=export_assets(root,directory,state)
    notes=directory/'RELEASE_NOTES.md';notes.write_text(release_notes((root/'CHANGELOG.md').read_text(),repository,tag))
    def gh(*cmd):return subprocess.check_output(['gh',*cmd],text=True)
    try:release=json.loads(gh('release','view',tag,'--repo',repository,'--json','isDraft,assets,url'))
    except subprocess.CalledProcessError:release=None
    if release and not release['isDraft']:
        if {x.name for x in assets}<={x['name'] for x in release['assets']}:
            print('Already published:',release['url']);return
        raise SystemExit('Refusing to modify an incomplete public release')
    if not release:
        gh('release','create',tag,'--repo',repository,'--target',target,'--title',f"{state['app']} {state['pkgver']}-{state['pkgrel']} · Arch Linux x86_64",'--draft','--notes-file',str(notes))
    else:
        gh('release','edit',tag,'--repo',repository,'--notes-file',str(notes))
        for item in release['assets']:
            if item['name']=='default.SRCINFO':gh('release','delete-asset',tag,item['name'],'--repo',repository,'--yes')
    gh('release','upload',tag,'--repo',repository,'--clobber',*[str(x) for x in assets])
    release=json.loads(gh('release','view',tag,'--repo',repository,'--json','assets'))
    actual={x['name']:x['size'] for x in release['assets']}
    for path in assets:
        if actual.get(path.name)!=path.stat().st_size:raise SystemExit(f'Incomplete asset upload: {path.name}')
    gh('release','edit',tag,'--repo',repository,'--draft=false','--latest=false')
    print(gh('release','view',tag,'--repo',repository,'--json','url','--jq','.url').strip())

def main():
    p=argparse.ArgumentParser();p.add_argument('directory');p.add_argument('--repository',required=True);p.add_argument('--target',required=True);args=p.parse_args()
    # Build artifacts can be read-only for the runner after container ownership changes.
    with tempfile.TemporaryDirectory(prefix='artcraft-release-') as directory:
        publish(pathlib.Path(args.directory),args.repository,args.target,pathlib.Path(directory))
if __name__=='__main__':main()
