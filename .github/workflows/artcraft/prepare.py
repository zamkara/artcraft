"""Pin upstream HEAD, skip already published inputs, and generate a PKGBUILD."""
import argparse,base64,datetime,hashlib,json,os,pathlib,re,shlex,time,tomllib
import urllib.error,urllib.parse,urllib.request
HERE=pathlib.Path(__file__).resolve().parent
API='https://api.github.com/'

def request(url,accept='application/vnd.github+json'):
    headers={'Accept':accept,'User-Agent':'artcraft-archlinux-workflow'}
    if url.startswith(API) and os.environ.get('GH_TOKEN'):
        headers['Authorization']='Bearer '+os.environ['GH_TOKEN']
    for attempt in range(4):
        try:
            with urllib.request.urlopen(urllib.request.Request(url,headers=headers),timeout=90) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            if error.code not in (429,500,502,503,504) or attempt==3:raise
        except (TimeoutError,urllib.error.URLError):
            if attempt==3:raise
        time.sleep(2**attempt)
    raise RuntimeError('Unreachable request')

def api(path):return json.loads(request(API+path))
def content(repo,path,sha):
    value=api(f'repos/{repo}/contents/{path}?ref={sha}')
    return base64.b64decode(value['content']).decode()

def releases(repo):
    page=1
    while True:
        values=api(f'repos/{repo}/releases?per_page=100&page={page}')
        yield from values
        if len(values)<100:break
        page+=1

def previous(repo,app):
    for release in releases(repo):
        if release['draft']:continue
        asset=next((x for x in release['assets'] if x['name']==app+'.upstream.json'),None)
        if asset is None and release['tag_name'].startswith(app+'-'):
            asset=next((x for x in release['assets'] if x['name']=='upstream.json'),None)
        if asset:
            state=json.loads(request(asset['url'],'application/octet-stream'))
            if state.get('app')!=app:raise ValueError('Incorrect release state')
            if not any(x['name']==state.get('package') for x in release['assets']):raise ValueError('Incomplete published release')
            return state
    return None

def recipe_hash():
    h=hashlib.sha256()
    for name in ('PKGBUILD.in','apps.json','build.sh','validate.py'):
        h.update(name.encode());h.update((HERE/name).read_bytes())
    return h.hexdigest()

def font_commit(workflow):
    if 'repository: storytold/craft-fonts' not in workflow:return ''
    match=re.search(r'CRAFT_FONTS_REF:\s*[\'"]?([0-9a-f]{40})',workflow)
    if not match:match=re.search(r'repository:\s*storytold/craft-fonts\s+ref:\s*[\'"]?([0-9a-f]{40})',workflow)
    if not match:raise ValueError('Upstream craft-fonts input must be pinned to a commit')
    return match.group(1)

def same_inputs(old,new):
    return old is not None and all(old.get(k)==new.get(k) for k in ('commit','fonts_commit','recipe_sha256'))

def pkgversion(version,stamp,sha):
    # Timestamp comes from the commit, not this workflow run, for deterministic rebuilds.
    base=re.sub(r'[^A-Za-z0-9.+_]','.',version)
    date=re.sub(r'[^0-9]','',stamp)[:14]
    return f'{base}.r{date}.g{sha[:12]}'

def changes(repo,base,head):
    result=[];page=1
    while True:
        data=api(f'repos/{repo}/compare/{base}...{head}?per_page=100&page={page}')
        commits=data.get('commits',[])
        result.extend(f"- {x['commit']['message']}\n  {x['html_url']}" for x in commits)
        if len(commits)<100:break
        page+=1
    return '\n\n'.join(result)

def notes(repo,sha,old):
    if old:
        text=changes(repo,old['commit'],sha)
        return text or f'https://github.com/{repo}/commit/{sha}\n'
    try:release=api(f'repos/{repo}/releases/latest')
    except urllib.error.HTTPError as error:
        if error.code!=404:raise
        release=None
    if release:
        text=release.get('body') or release['html_url']
        tag=release['tag_name']
        extra=changes(repo,tag,sha)
        return text+('\n\n'+extra if extra else '')+'\n'
    commits=api(f'repos/{repo}/commits?sha={sha}&per_page=100')
    return '\n\n'.join(f"- {x['commit']['message']}\n  {x['html_url']}" for x in reversed(commits))+'\n'

def download_archive(repo,sha,directory,name):
    url=f'https://codeload.github.com/{repo}/tar.gz/{sha}'
    path=directory/name
    data=request(url);path.write_bytes(data)
    return f'{name}::{url}',hashlib.sha256(data).hexdigest()

def output(values):
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'],'a') as stream:
            for key,value in values.items():stream.write(f'{key}={value}\n')
    print(json.dumps(values))

def main():
    parser=argparse.ArgumentParser();parser.add_argument('app');parser.add_argument('--repository',default=os.environ.get('GITHUB_REPOSITORY','zamkara/artcraft'));parser.add_argument('--directory',required=True)
    args=parser.parse_args();apps=json.loads((HERE/'apps.json').read_text())
    config=apps[args.app];repo=config['repo'];directory=pathlib.Path(args.directory);directory.mkdir(parents=True,exist_ok=True)
    info=api(f'repos/{repo}');commit=api(f'repos/{repo}/commits/{urllib.parse.quote(info["default_branch"],safe="")}')
    sha=commit['sha'];cargo=tomllib.loads(content(repo,'Cargo.toml',sha));version=cargo['workspace']['package']['version']
    workflow=content(repo,'.github/workflows/release.yml',sha);fonts=font_commit(workflow)
    state={'app':args.app,'upstream':repo,'branch':info['default_branch'],'commit':sha,'version':version,'commit_date':commit['commit']['committer']['date'],'fonts_commit':fonts,'recipe_sha256':recipe_hash(),'architecture':'x86_64'}
    old=previous(args.repository,args.app)
    if same_inputs(old,state):
        output({'changed':'false','app':args.app});return
    pkgver=pkgversion(version,state['commit_date'],sha)
    pkgrel=old.get('pkgrel',1)+1 if old and old.get('pkgver')==pkgver else 1
    state.update(pkgver=pkgver,pkgrel=pkgrel,previous_commit=old['commit'] if old else None)
    sources=[];checksums=[]
    source,checksum=download_archive(repo,sha,directory,f'{args.app}-{sha}.tar.gz');sources.append(source);checksums.append(checksum)
    if fonts:
        source,checksum=download_archive('storytold/craft-fonts',fonts,directory,f'craft-fonts-{fonts}.tar.gz');sources.append(source);checksums.append(checksum)
    # Follow official build features without adding a distro-specific fork.
    linux_script=content(repo,'packaging/linux/package.sh',sha)
    features=' --features heif' if '--features heif' in linux_script else ''
    replacements={'APP':args.app,'PKGVER':pkgver,'PKGREL':str(pkgrel),'DESCRIPTION':shlex.quote(config['description']),'URL':shlex.quote('https://github.com/'+repo),'FONT_LICENSE':" 'OFL-1.1'" if fonts else '', 'AUDIO':" 'alsa-lib'" if config['audio'] else '', 'SHA':sha,'FONT_SHA':shlex.quote(fonts),'UPSTREAM_VERSION':shlex.quote(version),'BUILD_DATE':state['commit_date'][:10],'SOURCES':'\n        '.join(shlex.quote(x) for x in sources),'CHECKSUMS':'\n            '.join(shlex.quote(x) for x in checksums),'ENV_PREFIX':args.app.upper(),'FEATURES':features}
    recipe=(HERE/'PKGBUILD.in').read_text()
    for key,value in replacements.items():recipe=recipe.replace('@'+key+'@',value)
    (directory/'PKGBUILD').write_text(recipe)
    (directory/'upstream.json').write_text(json.dumps(state,indent=2)+'\n')
    # Release notes consist of upstream release text and unedited upstream commit messages.
    (directory/'CHANGELOG.md').write_text(notes(repo,sha,old))
    output({'changed':'true','app':args.app,'tag':f'{args.app}-{pkgver}-{pkgrel}','pkgver':pkgver,'recipe':state['recipe_sha256']})
if __name__=='__main__':main()
