import json,pathlib,tempfile,unittest
from unittest.mock import patch
import prepare

class PlanningTests(unittest.TestCase):
    def test_identical_published_inputs_skip(self):
        state={'commit':'a'*40,'fonts_commit':'b'*40,'recipe_sha256':'c'*64}
        self.assertTrue(prepare.same_inputs(dict(state),state))
        self.assertFalse(prepare.same_inputs(None,state))
        for key in state:
            changed=dict(state);changed[key]='different'
            self.assertFalse(prepare.same_inputs(state,changed))

    def test_skip_happens_before_source_download_or_changelog_fetch(self):
        commit='a'*40;recipe='b'*64
        old={'app':'designcraft','commit':commit,'fonts_commit':'','recipe_sha256':recipe}
        def api(path):
            if path=='repos/storytold/designcraft':return {'default_branch':'main'}
            if path=='repos/storytold/designcraft/commits/main':return {'sha':commit,'commit':{'committer':{'date':'2026-10-08T12:34:56Z'}}}
            raise AssertionError('Unexpected request: '+path)
        def content(repo,path,sha):
            return '[workspace.package]\nversion="0.3.0"\n' if path=='Cargo.toml' else ''
        with tempfile.TemporaryDirectory() as directory,patch('sys.argv',['prepare.py','designcraft','--directory',directory]),patch.object(prepare,'api',api),patch.object(prepare,'content',content),patch.object(prepare,'recipe_hash',return_value=recipe),patch.object(prepare,'previous',return_value=old),patch.object(prepare,'download_archive') as archive,patch.object(prepare,'notes') as notes,patch.object(prepare,'output') as output:
            prepare.main();archive.assert_not_called();notes.assert_not_called()
            self.assertEqual(output.call_args.args[0]['changed'],'false')

    def test_snapshot_version_is_reproducible(self):
        self.assertEqual(prepare.pkgversion('0.3.0','2026-10-08T12:34:56Z','a'*40),'0.3.0.r20261008123456.gaaaaaaaaaaaa')

    def test_fonts_follow_upstream_pin(self):
        sha='a'*40
        self.assertEqual(prepare.font_commit('repository: storytold/craft-fonts\nref: '+sha),sha)
        self.assertEqual(prepare.font_commit('CRAFT_FONTS_REF: '+sha+'\nrepository: storytold/craft-fonts\nref: ${{ env.CRAFT_FONTS_REF }}'),sha)
        self.assertEqual(prepare.font_commit(''),'')
        with self.assertRaises(ValueError):prepare.font_commit('repository: storytold/craft-fonts\nref: main')

    def test_changelog_preserves_upstream_commit_messages(self):
        message='Fix a crash\n\nExact upstream description.'
        with patch.object(prepare,'api',return_value={'commits':[{'commit':{'message':message},'html_url':'https://github.com/owner/app/commit/abc'}]}):
            self.assertIn(message,prepare.changes('owner/app','abc','def'))

if __name__=='__main__':unittest.main()
