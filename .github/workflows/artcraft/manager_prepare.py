"""Prepare Artcraft's own Rust CLI independently of creative-app builds."""
import argparse,hashlib,json,os,pathlib,shlex,tomllib
from prepare import api,download_archive,output,pkgversion,previous
HERE=pathlib.Path(__file__).resolve().parent

def fingerprint():
    files=list((HERE/'cli').rglob('*.rs'))+[HERE/'cli/Cargo.toml',HERE/'cli/Cargo.lock',HERE/'install.sh.in',HERE/'manager.PKGBUILD.in',HERE/'manager-build.sh',HERE/'manager_prepare.py',HERE.parents[2]/'LICENSE']
    digest=hashlib.sha256()
    for path in sorted(files):
        digest.update(path.name.encode());digest.update(path.read_bytes())
    return digest.hexdigest()

def main():
    p=argparse.ArgumentParser();p.add_argument('--directory',required=True);args=p.parse_args()
    repo=os.environ['GITHUB_REPOSITORY'];sha=os.environ['GITHUB_SHA'];directory=pathlib.Path(args.directory);directory.mkdir(parents=True,exist_ok=True)
    recipe=fingerprint();old=previous(repo,'artcraft')
    if old and old.get('manager_fingerprint')==recipe:
        output({'changed':'false'});return
    commit=api(f'repos/{repo}/commits/{sha}')
    version=tomllib.loads((HERE/'cli/Cargo.toml').read_text())['package']['version'];stamp=commit['commit']['committer']['date']
    pkgver=pkgversion(version,stamp,sha);tag=f'artcraft-{pkgver}-1'
    source,checksum=download_archive(repo,sha,directory,f'artcraft-{sha}.tar.gz')
    template=(HERE/'manager.PKGBUILD.in').read_text()
    for key,value in {'PKGVER':pkgver,'SHA':sha,'SOURCE':shlex.quote(source),'CHECKSUM':checksum,'SOURCE_DIR':repo.split('/')[-1]}.items():template=template.replace('@'+key+'@',value)
    (directory/'PKGBUILD').write_text(template)
    (directory/'install.sh').write_text((HERE/'install.sh.in').read_text().replace('@RELEASE_TAG@',tag))
    state={'app':'artcraft','upstream':repo,'commit':sha,'version':version,'pkgver':pkgver,'pkgrel':1,'architecture':'x86_64','manager_fingerprint':recipe}
    (directory/'upstream.json').write_text(json.dumps(state,indent=2)+'\n')
    notes=f'Install, inspect, update, and remove native creative-app packages through pacman.\n\nSource: https://github.com/{repo}/commit/{sha}\n'
    (directory/'CHANGELOG.md').write_text(notes)
    output({'changed':'true','tag':tag,'recipe':recipe})
if __name__=='__main__':main()
