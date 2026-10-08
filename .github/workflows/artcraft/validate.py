"""Check package metadata, integration files, and ELF runtime dependencies."""
import hashlib,json,pathlib,subprocess,sys,tarfile
root=pathlib.Path(sys.argv[1]);state=json.loads((root/'upstream.json').read_text())
app=state['app'];packages=list(root.glob('*.pkg.tar.zst'))
if len(packages)!=1:raise SystemExit(f'Expected one package, found {len(packages)}')
package=packages[0]
subprocess.run(['pacman','-Qip',str(package)],check=True)
listing=subprocess.check_output(['bsdtar','-tf',str(package)],text=True).splitlines()
required=['.PKGINFO','.BUILDINFO',f'usr/bin/{app}',f'usr/bin/{app}-cli',f'usr/share/applications/ai.storyteller.{app}.desktop',f'usr/share/mime/packages/ai.storyteller.{app}.xml',f'usr/share/licenses/{app}/LICENSE-MIT',f'usr/share/licenses/{app}/LICENSE-APACHE']
for name in required:
 if name not in listing:raise SystemExit(f'Missing package member: {name}')
for binary in [root/'.cache/target/release'/app,root/'.cache/target/release'/f'{app}-cli']:
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
