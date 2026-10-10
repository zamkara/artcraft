import json,pathlib,tempfile,unittest
from unittest.mock import patch
import registry,prepare
class RegistryTests(unittest.TestCase):
    def test_registry_contains_all_fourteen_upstream_apps(self):
        apps=registry.load_apps()
        self.assertEqual(len(apps),14)
        self.assertNotIn('artcraft',apps)
        self.assertEqual(apps['artcraft-studio']['repo'],'storytold/artcraft')
        self.assertNotIn('artcraft-launcher',apps)
    def test_manager_name_is_reserved(self):
        with tempfile.TemporaryDirectory() as folder:
            path=pathlib.Path(folder)/'apps.json';path.write_text(json.dumps({'artcraft':{'repo':'storytold/artcraft','description':'App'}}))
            with self.assertRaises(ValueError):registry.load_apps(path)
    def test_tauri_version_comes_from_upstream_app_configuration(self):
        with patch.object(prepare,'content',return_value=json.dumps({'version':'0.42.0','identifier':'ai.artcraft.app'})):
            version,app_id,fonts=prepare.build_metadata({'kind':'tauri','crate_path':'crates/desktop/artcraft'},{'workspace':{}},'storytold/artcraft','commit')
            self.assertEqual((version,app_id,fonts),('0.42.0','ai.artcraft.app',''))
    def test_recipes_support_tauri_without_a_cli_binary(self):
        for name in ['artcraft-studio','artcraftx']:
            config=registry.load_apps()[name];repo=config['repo'];sha='a'*40
            def api(path):
                if path=='repos/'+repo:return {'default_branch':'main'}
                if path=='repos/'+repo+'/commits/main':return {'sha':sha,'commit':{'committer':{'date':'2026-10-10T00:00:00Z'}}}
                raise AssertionError(path)
            def content(repo,path,sha):
                if path=='Cargo.toml':return '[workspace.package]\nversion="0.1.3"\n'
                if path.endswith('tauri.conf.json'):return json.dumps({'version':'0.42.0','identifier':'ai.artcraft.app'})
                return ''
            with tempfile.TemporaryDirectory() as folder,patch('sys.argv',['prepare.py',name,'--directory',folder]),patch.object(prepare,'api',api),patch.object(prepare,'content',content),patch.object(prepare,'previous',return_value=None),patch.object(prepare,'download_archive',return_value=('source.tar.gz::https://example.com/source','b'*64)),patch.object(prepare,'notes',return_value='Exact upstream notes'),patch.object(prepare,'output'):
                prepare.main();recipe=(pathlib.Path(folder)/'PKGBUILD').read_text()
                self.assertNotIn('usr/bin/'+name+'-cli',recipe)
                self.assertIn(repo.split('/')[-1]+'-$_commit',recipe)
                import subprocess,re
                self.assertFalse(set(re.findall(r'@[A-Z_]+@',recipe)) - {'@VERSION@','@DATE@'})
                subprocess.run(['bash','-n',str(pathlib.Path(folder)/'PKGBUILD')],check=True)
                if config.get('kind')=='tauri':
                    self.assertIn('npm ci',recipe);self.assertIn('tauri/custom-protocol',recipe)
                    self.assertIn('nx run artcraft:build',recipe);self.assertIn('--allow-git=all',recipe)
