"""Publish all supported verified packages together; never expose a partial suite."""
import argparse,hashlib,json,os,pathlib,subprocess,tempfile
from prepare import api,request,releases,HERE
from registry import load_apps
APPS=tuple(load_apps())+('artcraft',)

def digest(path):
    value=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b''):value.update(chunk)
    return value.hexdigest()

def remote_state(repository,app):
    for release in releases(repository):
        if release['draft'] or release['prerelease']:continue
        names={x['name']:x for x in release['assets']}
        name=app+'.upstream.json'
        if name not in names:
            if not release['tag_name'].startswith(app+'-') or 'upstream.json' not in names:continue
            name='upstream.json'
        state=json.loads(request(names[name]['browser_download_url'],'application/octet-stream'))
        if state.get('app')==app and state.get('package') in names and 'SHA256SUMS' in names:
            return state,names
    raise RuntimeError(f'Missing validated build or published package for {app}; suite publication blocked')

def collect(artifacts,directory,repository):
    built={}
    for manifest in artifacts.rglob('upstream.json'):
        state=json.loads(manifest.read_text());app=state['app']
        if app not in APPS:raise ValueError('Unexpected application: '+app)
        if app in built:raise ValueError('Duplicate build: '+app)
        subprocess.run(['sha256sum','--check','SHA256SUMS'],cwd=manifest.parent,check=True)
        built[app]=(state,manifest.parent)
    states={}
    for app in APPS:
        if app in built:
            state,root=built[app]
            files={state['package']:root/state['package'],app+'.PKGBUILD':root/'PKGBUILD',app+'.SRCINFO':root/'.SRCINFO',app+'.upstream.json':root/'upstream.json',app+'.CHANGELOG.md':root/'CHANGELOG.md'}
            if app=='artcraft':files['artcraft-linux-x86_64']=root/'artcraft-linux-x86_64'
            for name,path in files.items():(directory/name).write_bytes(path.read_bytes())
        else:
            state,assets=remote_state(repository,app)
            sums=request(assets['SHA256SUMS']['browser_download_url'],'application/octet-stream').decode()
            checks={line.split()[1]:line.split()[0] for line in sums.splitlines() if len(line.split())==2}
            files={state['package']:state['package']}
            for suffix,legacy in [('PKGBUILD','PKGBUILD'),('SRCINFO','SRCINFO'),('upstream.json','upstream.json'),('CHANGELOG.md','CHANGELOG.md')]:
                name=app+'.'+suffix;files[name]=name if name in assets else legacy
            if app=='artcraft':files['artcraft-linux-x86_64']='artcraft-linux-x86_64'
            for name,old in files.items():
                if old not in assets or old not in checks:raise ValueError('Incomplete published input: '+old)
                path=directory/name;path.write_bytes(request(assets[old]['browser_download_url'],'application/octet-stream'))
                if digest(path)!=checks[old]:raise ValueError('Published input checksum mismatch: '+old)
        states[app]=state
    return states,bool(built)

def publish(artifacts,repository,target,directory):
    if not any(artifacts.rglob('upstream.json')):
        for app in APPS:remote_state(repository,app)
        print(f'All {len(APPS)} packages are current; no downloads or release needed');return
    states,changed=collect(artifacts,directory,repository)
    if not changed:
        print(f'All {len(APPS)} packages are current; suite release skipped');return
    run=os.environ['GITHUB_RUN_ID']
    tag=f'artcraft-suite-r{run}.g{target[:12]}'
    (directory/'install.sh').write_text((HERE/'install.sh.in').read_text().replace('@RELEASE_TAG@',tag))
    config=load_apps()
    catalog_states={app:{**state,'description':config.get(app,{}).get('description','Native Arch application manager')} for app,state in states.items()}
    manifest={'repository':repository,'target':target,'apps':catalog_states}
    (directory/'suite.json').write_text(json.dumps(manifest,indent=2)+'\n')
    paths=sorted(directory.iterdir())
    with (directory/'SHA256SUMS').open('w') as stream:
        for path in paths:stream.write(f'{digest(path)}  {path.name}\n')
    paths.append(directory/'SHA256SUMS')
    notes=f'Artcraft CLI and all supported Storytold native applications for Arch Linux and Omarchy.\n\n```sh\ncurl -fsSL https://github.com/{repository}/releases/latest/download/install.sh | bash\nartcraft -ia\n```\n\n'
    for app,state in states.items():
        notes+=f"- **{app}** {state['pkgver']}-{state['pkgrel']} · [Complete changelog](https://github.com/{repository}/releases/download/{tag}/{app}.CHANGELOG.md)\n"
    for app in APPS:
        if app=='artcraft':continue
        text=(directory/(app+'.CHANGELOG.md')).read_text()
        remaining=110000-len(notes)
        if remaining>3000:
            excerpt=text if len(text)<remaining-500 else text[:remaining-500].rsplit('\n\n',1)[0]
            notes+=f'\n## {app}\n\n{excerpt}\n'
    notesfile=directory/'RELEASE_NOTES.md';notesfile.write_text(notes)
    def gh(*args):return subprocess.check_output(['gh',*args],text=True)
    try:release=json.loads(gh('release','view',tag,'--repo',repository,'--json','isDraft,assets,url'))
    except subprocess.CalledProcessError:release=None
    if release and not release['isDraft']:
        if {p.name for p in paths}<={a['name'] for a in release['assets']}:
            print('Already published:',release['url']);return
        raise RuntimeError('Refusing to modify an incomplete public suite')
    if not release:gh('release','create',tag,'--repo',repository,'--target',target,'--title','Storytold · Complete native application suite','--draft','--notes-file',str(notesfile))
    else:gh('release','edit',tag,'--repo',repository,'--notes-file',str(notesfile))
    gh('release','upload',tag,'--repo',repository,'--clobber',*[str(p) for p in paths])
    actual=json.loads(gh('release','view',tag,'--repo',repository,'--json','assets'))
    sizes={a['name']:a['size'] for a in actual['assets']}
    for path in paths:
        if sizes.get(path.name)!=path.stat().st_size:raise RuntimeError('Incomplete upload: '+path.name)
    gh('release','edit',tag,'--repo',repository,'--draft=false','--latest=true')
    print(f'https://github.com/{repository}/releases/tag/{tag}')

def main():
    p=argparse.ArgumentParser();p.add_argument('--artifacts',type=pathlib.Path,required=True);p.add_argument('--repository',required=True);p.add_argument('--target',required=True);args=p.parse_args()
    with tempfile.TemporaryDirectory(prefix='artcraft-suite-') as directory:
        publish(args.artifacts,args.repository,args.target,pathlib.Path(directory))
if __name__=='__main__':main()
