import hashlib,json,pathlib,tempfile,unittest
from unittest.mock import patch
from suite_publish import APPS,collect,publish
class SuiteTests(unittest.TestCase):
    def fixture(self,root,app):
        folder=root/app;folder.mkdir()
        package=app+'-1.0-1-x86_64.pkg.tar.zst'
        state={'app':app,'package':package,'pkgver':'1.0','pkgrel':1}
        names=[package,'PKGBUILD','.SRCINFO','CHANGELOG.md']
        if app=='artcraft':names+=['artcraft-linux-x86_64']
        for name in names:(folder/name).write_text(name+' content')
        (folder/'upstream.json').write_text(json.dumps(state));names+=['upstream.json']
        (folder/'SHA256SUMS').write_text(''.join(hashlib.sha256((folder/name).read_bytes()).hexdigest()+'  '+name+'\n' for name in names))
    def test_complete_suite_collects_all_eight_without_metadata_collisions(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp);artifacts=root/'artifacts';artifacts.mkdir();out=root/'out';out.mkdir()
            for app in APPS:self.fixture(artifacts,app)
            states,changed=collect(artifacts,out,'owner/repo')
            self.assertTrue(changed);self.assertEqual(set(states),set(APPS))
            for app in APPS:
                self.assertTrue((out/states[app]['package']).is_file())
                self.assertEqual(json.loads((out/(app+'.upstream.json')).read_text())['app'],app)
    def test_missing_app_blocks_publication(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp);artifacts=root/'artifacts';artifacts.mkdir();out=root/'out';out.mkdir()
            self.fixture(artifacts,'artcraft')
            with patch('suite_publish.remote_state',side_effect=RuntimeError('Missing app')):
                with self.assertRaises(RuntimeError):collect(artifacts,out,'owner/repo')
    def test_unchanged_suite_skips_package_downloads(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp)
            with patch('suite_publish.remote_state') as remote,patch('suite_publish.collect',side_effect=AssertionError('Must not download unchanged packages')):
                publish(root,'owner/repo','commit',root)
                self.assertEqual(remote.call_count,len(APPS))
    def test_corrupt_build_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp);artifacts=root/'artifacts';artifacts.mkdir();out=root/'out';out.mkdir()
            self.fixture(artifacts,'artcraft');(artifacts/'artcraft/PKGBUILD').write_text('corrupt')
            with self.assertRaises(Exception):collect(artifacts,out,'owner/repo')
