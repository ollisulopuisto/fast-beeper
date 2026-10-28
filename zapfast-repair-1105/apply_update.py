#!/usr/bin/env python3
"""Hash-checked update for delivered0855source. Refuses edited files before any writes."""
import argparse,hashlib,json,pathlib,shutil,subprocess,platform
p=argparse.ArgumentParser();p.add_argument('--root',type=pathlib.Path,required=True);p.add_argument('--build',action='store_true');a=p.parse_args();root=a.root.resolve();here=pathlib.Path(__file__).resolve().parent;manifest=json.loads((here/'manifest.json').read_text())
for name,item in manifest.items():
 path=root/name;current=hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
 if current not in (item['before'],item['after']):raise SystemExit('Source differs from delivered0855; no files changed: '+name)
 if hashlib.sha256((here/'files'/name).read_bytes()).hexdigest()!=item['after']:raise SystemExit('Update integrity check failed: '+name)
backup=root/'repair-1105-backup'
for name,item in manifest.items():
 path=root/name
 if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest()==item['after']:continue
 if path.exists():
  dest=backup/name;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,dest)
 path.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(here/'files'/name,path)
print('Repair applied. Original changed files kept in repair-1105-backup. No accounts or messages changed.')
if a.build:
 if platform.system()!='Darwin':raise SystemExit('Native build requires Mac; source update applied, no build attempted.')
 target={'arm64':'aarch64-apple-darwin','x86_64':'x86_64-apple-darwin'}[platform.machine()]
 subprocess.run(['cargo','build','--locked','--release','--bin','zapfast','--target',target,'--target-dir',str(root/'target')],cwd=root,check=True)
 print('Native build completed. Existing dist trial binary is unchanged; launch the newly built target binary.')
