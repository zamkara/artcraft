"""Validate the supported application registry and emit the CI matrix."""
import json,pathlib,re
HERE=pathlib.Path(__file__).resolve().parent

def load_apps(path=None):
    apps=json.loads((path or HERE/'apps.json').read_text())
    if not isinstance(apps,dict) or not apps:raise ValueError('Application registry must be a nonempty object')
    for name,app in apps.items():
        if name=='artcraft' or not re.fullmatch(r'[a-z0-9][a-z0-9-]*',name):raise ValueError('Invalid or reserved package name: '+name)
        if not re.fullmatch(r'storytold/[A-Za-z0-9_.-]+',app.get('repo','')):raise ValueError('Invalid upstream repository: '+name)
        if not app.get('description'):raise ValueError('Missing description: '+name)
        if app.get('kind','craft') not in ('craft','tauri'):raise ValueError('Unknown builder: '+name)
        if app.get('kind')=='tauri' and not app.get('crate_path'):raise ValueError('Missing Tauri crate path: '+name)
    return apps

if __name__=='__main__':print('apps='+json.dumps(list(load_apps()),separators=(',',':')))
