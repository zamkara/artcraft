import unittest,tempfile,pathlib,hashlib
from publish import release_notes,export_assets
class ReleaseNotesTests(unittest.TestCase):
    def test_release_exports_visible_srcinfo_and_matching_checksums(self):
        with tempfile.TemporaryDirectory() as rootdir,tempfile.TemporaryDirectory() as outdir:
            root=pathlib.Path(rootdir);out=pathlib.Path(outdir)
            for name in ['app.pkg.tar.zst','PKGBUILD','.SRCINFO','upstream.json','CHANGELOG.md']:
                (root/name).write_bytes(('content '+name).encode())
            assets=export_assets(root,out,{'package':'app.pkg.tar.zst'})
            self.assertEqual((out/'SRCINFO').read_bytes(),(root/'.SRCINFO').read_bytes())
            self.assertNotIn('.SRCINFO',[x.name for x in assets])
            checks={line.split('  ')[1]:line.split('  ')[0] for line in (out/'SHA256SUMS').read_text().splitlines()}
            for path in assets[:-1]:self.assertEqual(checks[path.name],hashlib.sha256(path.read_bytes()).hexdigest())
    def test_short_notes_are_unchanged(self):
        body='Exact upstream text.\n\nSecond paragraph.\n'
        self.assertEqual(release_notes(body,'owner/repo','tag'),body)
    def test_long_notes_link_full_original_asset(self):
        body=('An original upstream paragraph.\n\n'*10000)
        result=release_notes(body,'owner/repo','tag')
        excerpt=result.split('[Complete upstream changelog]')[0].rstrip()
        self.assertTrue(body.startswith(excerpt))
        self.assertLess(len(result),125000)
        self.assertIn('/releases/download/tag/CHANGELOG.md',result)
if __name__=='__main__':unittest.main()
