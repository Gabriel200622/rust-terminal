#!/usr/bin/env python3
"""Focused native agent restoration proof using isolated, deterministic CLI fixtures.
Real provider hook behavior requires a separate installed-CLI smoke test.
"""
import json
import os
from pathlib import Path
import runpy
import shlex
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
H = runpy.run_path(str(ROOT / 'scripts/native-harness.py'))
APP = ROOT / 'target/debug/neptune'
CLIENT = ROOT / 'target/debug/neptune-inspect'
FIXTURE = r'''#!/usr/bin/env python3
import json, os, shlex, signal, subprocess, sys, tomllib, uuid
from pathlib import Path
args = sys.argv[1:]
provider = Path(sys.argv[0]).name
if '--help' in args:
    print('--no-daemon'); sys.exit(0)
if provider == 'claude':
    hook = json.loads(args[args.index('--settings')+1])['hooks']['SessionStart'][0]['hooks'][0]['command']
    flag = '--resume'
else:
    hook = tomllib.loads(args[args.index('-c')+1])['hooks']['SessionStart'][0]['hooks'][0]['command']
    flag = 'resume'
resumed = flag in args
session = args[args.index(flag)+1] if resumed else str(uuid.uuid4())
subprocess.run(shlex.split(hook), input=json.dumps({'session_id':session,'cwd':os.getcwd(),'hook_event_name':'SessionStart'}), text=True, check=True)
with open(os.environ['NEPTUNE_AGENT_TEST_LOG'], 'a') as log: log.write(json.dumps({'provider':provider,'session':session,'resumed':resumed})+'\n')
print('\033[2J\033[H' + provider.upper() + (' RESUMED ' if resumed else ' SESSION ') + session, flush=True)
def interrupt(*_):
    with open(os.environ['NEPTUNE_AGENT_TEST_LOG'], 'a') as log: log.write(json.dumps({'interrupt':provider})+'\n')
    print('INTERRUPT HANDLED', flush=True)
signal.signal(signal.SIGINT, interrupt)
while True:
    line = sys.stdin.readline()
    if not line or line.strip() == 'exit': break
    print('AGENT INPUT: ' + line.strip(), flush=True)
'''

def main():
    output = Path(tempfile.mkdtemp(prefix='agent-restore-', dir=ROOT/'artifacts')).resolve()
    data = output / 'data'; data.mkdir()
    home = output / 'home'; home.mkdir()
    fixtures = output / 'bin'; fixtures.mkdir()
    for provider in ('codex', 'claude'):
        path = fixtures/provider; path.write_text(FIXTURE); path.chmod(0o700)
    (data/'config.toml').write_text('shell = "/bin/bash"\nconfirm_close = false\nwarn_running_processes = false\n')
    (home/'.bashrc').write_text(f'export PATH={shlex.quote(str(fixtures))}:"$PATH"\nPS1="test $ "\n')
    endpoint = H['free_endpoint']()
    env = os.environ.copy(); env.pop('WAYLAND_DISPLAY',None)
    env.update(HOME=str(home), EGUI_INSPECTION=endpoint, NEPTUNE_AGENT_TEST_LOG=str(output/'events.jsonl'))
    env['PATH'] = str(fixtures) + ':' + env['PATH']
    process = None
    log = (output/'app.log').open('w')
    def call(*args): return H['inspect'](CLIENT, endpoint, *map(str,args))
    def wait(predicate):
        end=time.monotonic()+15
        while time.monotonic()<end:
            try:
                value=predicate()
                if value: return value
            except (OSError, ValueError, KeyError): pass
            time.sleep(.05)
        raise AssertionError('Timed out waiting for native agent state')
    def saved(): return json.loads((data/'workspaces.json').read_text())
    def agents(): return [p.get('agent') for w in saved()['workspaces'] for p in w['panes']]
    def text(value): call('text',value); call('key','Enter')
    def launch(restore):
        nonlocal process
        args=[str(APP),'--data-root',str(data),'--size','1100x700']
        if not restore: args += ['--cwd', str(data), '--no-restore']
        process=subprocess.Popen(args,env=env,stdout=log,stderr=log)
        H['wait_ready'](process,CLIENT,endpoint,30)
    def close():
        nodes=call('tree')['Tree']['accesskit']['nodes']
        button=next(n for _,n in nodes if n['properties'].get('label')=='Close window')
        bounds=button['properties']['bounds']
        call('click',(bounds['x0']+bounds['x1'])/2,(bounds['y0']+bounds['y1'])/2)
        process.wait(timeout=10)
    try:
        launch(False)
        call('screenshot',output/'before.png')
        text('claude')
        wait(lambda: len(agents())==1 and agents()[0] and agents()[0]['session_id'])
        call('key','d',*(['--cmd'] if sys.platform == 'darwin' else ['--ctrl','--shift']))
        wait(lambda: len(agents())==2)
        text('codex')
        wait(lambda: all(a and a['session_id'] for a in agents()))
        original=agents()
        assert original[0]['session_id'] != original[1]['session_id']
        call('screenshot',output/'running.png')
        close()
        assert agents()==original
        launch(True)
        wait(lambda: agents()==original)
        # Wait for visible terminal output, not just the loaded saved references.
        def resumed_text():
            events=[json.loads(line) for line in (output/'events.jsonl').read_text().splitlines()]
            return len([event for event in events if event.get('resumed')]) >= 2
        wait(resumed_text)
        call('screenshot',output/'restored.png')
        call('resize',640,440)
        call('screenshot',output/'restored-narrow.png')
        call('key','c','--ctrl')
        wait(lambda: 'interrupt' in (output/'events.jsonl').read_text())
        assert agents()==original
        text('exit')
        wait(lambda: agents()[1] is None)
        close()
        launch(True)
        wait(lambda: sum(json.loads(line).get('resumed',False) for line in (output/'events.jsonl').read_text().splitlines()) >= 3)
        assert agents()[1] is None
        call('screenshot',output/'agent-exited.png')
        close()
        (output/'result.json').write_text(json.dumps({'status':'passed','platform':sys.platform,'features':['inspection'],'provider':'deterministic fixtures','checks':['two providers in same directory','exact IDs after graceful close/reopen','Ctrl+C preserves agent ownership','normal agent exit returns to shell','exited agent remains shell after reopen'],'address':endpoint},indent=2))
        print(output)
    except Exception:
        if process and process.poll() is None:
            call('screenshot',output/'failure.png')
        print(output)
        raise
    finally:
        if process: H['stop_owned'](process)
        log.close()

if __name__=='__main__': main()
