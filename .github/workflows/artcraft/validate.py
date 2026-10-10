"""Check package metadata, integration files, and ELF runtime dependencies."""
import hashlib,json,pathlib,subprocess,sys,tarfile
root=pathlib.Path(sys.argv[1]);state=json.loads((root/'upstream.json').read_text())
app=state['app'];packages=list(root.glob('*.pkg.tar.zst'))
if len(packages)!=1:raise SystemExit(f'Expected one package, found {len(packages)}')
package=packages[0]
subprocess.run(['pacman','-Qip',str(package)],check=True)
listing=subprocess.check_output(['bsdtar','-tf',str(package)],text=True).splitlines()
app_id=state.get('app_id','ai.storyteller.'+app)
binary=state.get('binary',app)
required=['.PKGINFO','.BUILDINFO',f'usr/bin/{app}',f'usr/share/applications/{app_id}.desktop']
binaries=[root/'.cache/target/release'/binary]
if state.get('cli',True):
 required.append(f'usr/bin/{app}-cli');binaries.append(root/'.cache/target/release'/f'{binary}-cli')
if state.get('mime',True):required.append(f'usr/share/mime/packages/{app_id}.xml')
if state.get('kind')=='tauri':required.append(f'usr/lib/{app}/{binary}')
if app!='artcraftx':required.extend([f'usr/share/licenses/{app}/LICENSE-MIT',f'usr/share/licenses/{app}/LICENSE-APACHE'])
for name in required:
 if name not in listing:raise SystemExit(f'Missing package member: {name}')
for binary in binaries:
 result=subprocess.run(['ldd',str(binary)],capture_output=True,text=True)
 if result.returncode or 'not found' in result.stdout:raise SystemExit(f'Unresolved dependencies: {result.stdout}\n{result.stderr}')
state['package']=package.name
(root/'upstream.json').write_text(json.dumps(state,indent=2)+'\n')
files=[package,root/'PKGBUILD',root/'.SRCINFO',root/'upstream.json']
with (root/'SHA256SUMS').open('w') as stream:
 for path in files:
  digest=hashlib.sha256()
  with path.open('rb') as f:
   for block in iter(lambda:f.read(1024*1024),b''):digest.update(block)
  stream.write(f'{digest.hexdigest()}  {path.name}\n')
print('Validated native Arch package:',package.name)
