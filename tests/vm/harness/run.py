#!/usr/bin/env python3
"""Run repository-owned desktop suites through the local Oligarchy installation."""
import argparse
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import socket
import subprocess as sp
import sys
import tarfile
import time
import uuid
from images import resolve_image, sha256

ROOT = Path(__file__).resolve().parent
DEFAULTS = {name: Path.home() / 'Work/projects' / directory for name, directory in
            [('snipexpand', 'snipexpand-authoring'), ('look-elsewhere', 'look-elsewhere')]}
PG = Path(os.environ.get('OLIGARCHY_PG_BIN', str(Path.home()/'.local/share/mise/installs/postgres/18.4/bin')))


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def command(args, **kwargs):
    return sp.run([str(a) for a in args], check=True, timeout=kwargs.pop('timeout', 120), **kwargs)


class Suite:
    def __init__(self, name, source, out, image=None):
        self.name, self.source, self.out = name, source, out
        self.agent = f'{name}-{uuid.uuid4()}'
        self.session = None
        self.checks = []
        self.image = image
        self.ssh_args = ['ssh', '-i', str(ROOT.parent/'vm/state/id_ed25519'), '-p', '24223',
                         '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=3', '-o', 'StrictHostKeyChecking=yes',
                         '-o', f'UserKnownHostsFile={ROOT}/state/known_hosts', 'omarchy@127.0.0.1']

    def client(self, *args):
        argv = [sys.executable, ROOT/'with-env.py', 'bun', '--no-env-file', 'src/client/main.ts',
                *args, '--agent-id', self.agent]
        if args[0] not in ('reserve', 'start', 'relinquish'):
            argv += ['--session-id', self.session]
        return command(argv, capture_output=True, text=True).stdout.strip()

    def ssh(self, script, *, timeout=180, input=None):
        return command([*self.ssh_args, script], input=input, capture_output=True, timeout=timeout).stdout

    def remote(self, *args, timeout=180):
        return self.ssh(shlex.join(str(a) for a in args), timeout=timeout).decode()

    def screenshot(self, name):
        self.client('get-image', '-o', str(self.out/(name+'.png')))

    def record(self, name, passed, **details):
        self.checks.append({'name': name, 'passed': bool(passed), **details})
        write_json(self.out/'checks.json', self.checks)
        if not passed:
            raise RuntimeError(f'{name} failed; see {self.out}')

    def logged(self, name, script, timeout=180):
        try:
            output = self.ssh(script, timeout=timeout)
        except sp.CalledProcessError as e:
            (self.out/(name+'.log')).write_bytes((e.stdout or b'')+(e.stderr or b''))
            raise
        (self.out/(name+'.log')).write_bytes(output)
        return output

    def ready(self):
        deadline = time.monotonic()+120
        while time.monotonic() < deadline:
            try:
                self.remote('python3', '-c', "import os,json,subprocess; os.environ['XDG_RUNTIME_DIR']='/run/user/1000'; assert open('/etc/snipexpand-test-vm').read().strip()=='snipexpand-test-vm'; assert subprocess.check_output(['systemd-detect-virt','--vm']).strip() in (b'kvm',b'qemu'); assert len(json.loads(subprocess.check_output(['hyprctl','instances','-j'])))==1", timeout=8)
                return
            except (sp.SubprocessError, OSError):
                time.sleep(1)
        raise TimeoutError('Guest SSH/graphical session not ready after 120 seconds')

    def payload(self, adapter):
        files = []
        for relative in adapter.PAYLOAD:
            path = self.source/relative
            if not path.exists():
                raise FileNotFoundError(path)
            files.extend([p for p in path.rglob('*') if p.is_file()] if path.is_dir() else [path])
        files = sorted(set(p for p in files if '__pycache__' not in p.parts and p.suffix != '.pyc' and p.name != 'AGENTS.md'))
        manifest = {'source': str(self.source), 'commit': command(['git','-C',self.source,'rev-parse','HEAD'], capture_output=True,text=True).stdout.strip(),
                    'status': command(['git','-C',self.source,'status','--short'],capture_output=True,text=True).stdout,
                    'harness_sha256': {str(p): sha256(p) for p in
                        [ROOT/'run.py', ROOT/'images.py', ROOT/'guest-desktop.py', ROOT/'native-setup.py',
                         ROOT.parent/'vm/guest-test.py', ROOT.parent/'vm/guest-setup.py']}, 'sha256':{}}
        with tarfile.open(self.out/'payload.tar','w') as tar:
            for path in files:
                if path.is_symlink():
                    raise ValueError(f'Payload symlink refused: {path}')
                relative = path.relative_to(self.source).as_posix()
                data = path.read_bytes()
                manifest['sha256'][relative] = hashlib.sha256(data).hexdigest()
                # Hash and archive the same bytes, including local uncommitted work.
                import io
                info = tar.gettarinfo(str(path), arcname=self.name+'/'+relative)
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
            for name in ('guest-test.py','guest-setup.py'):
                tar.add(ROOT.parent/'vm'/name, arcname=name)
            tar.add(ROOT/'guest-desktop.py', arcname='guest-desktop.py')
            tar.add(ROOT/'native-setup.py', arcname='native-setup.py')
        write_json(self.out/'payload-manifest.json', manifest)
        with (self.out/'payload.tar').open('rb') as f:
            command([*self.ssh_args, 'tar -xf - -C "$HOME"'], stdin=f, capture_output=True)

    def execute(self, adapter):
        verdict = {'project': self.name, 'passed': False, 'artifacts': str(self.out)}
        reserved = False
        try:
            build_env = os.environ.copy()
            build_tmp = Path.home()/'.cache/sx-oligarchy'/('build-'+uuid.uuid4().hex[:8])
            build_tmp.mkdir()
            build_env['TMPDIR'] = str(build_tmp)
            for index, argv in enumerate(adapter.BUILD):
                with (self.out/f'build-{index}.log').open('wb') as log:
                    command(argv, cwd=self.source, env=build_env, stdout=log, stderr=sp.STDOUT, timeout=1200)
            iso, image_manifest = resolve_image(self.image)
            verdict['image'] = image_manifest
            self.client('reserve')
            reserved = True
            self.session = self.client('start','--iso',str(iso),'--resume')
            uuid.UUID(self.session)
            verdict['session'] = self.session
            self.ready()
            self.payload(adapter)
            self.screenshot('boot')
            adapter.run(self)
            verdict['passed'] = bool(self.checks) and all(c['passed'] for c in self.checks)
        except Exception as e:
            verdict['error'] = f'{type(e).__name__}: {e}'
            if isinstance(e, sp.CalledProcessError):
                (self.out/'error-output.txt').write_bytes((e.stdout or b'') if isinstance(e.stdout, bytes) else (e.stdout or '').encode())
                (self.out/'error-stderr.txt').write_bytes((e.stderr or b'') if isinstance(e.stderr, bytes) else (e.stderr or '').encode())
        finally:
            cleanup_errors = []
            if self.session:
                for action in [lambda: self.screenshot('final'),
                               lambda: (self.out/'guest-results.tar').write_bytes(self.ssh('tar -cf - -C "$HOME" test-results')),
                               lambda: self.ssh('sync')]:
                    try: action()
                    except Exception as e: cleanup_errors.append(str(e))
                try:
                    self.client('stop','--status','succeeded' if verdict['passed'] and not cleanup_errors else 'failed',
                                '--reason',f'{self.name}: see saved checks and guest evidence')
                except Exception as e:
                    cleanup_errors.append(str(e))
                try:
                    result=command([sys.executable,ROOT/'with-env.py','./ctrl','session','--session-id',self.session,'--all'],capture_output=True,text=True)
                    (self.out/'oligarchy-session.json').write_text(result.stdout)
                except Exception as e: cleanup_errors.append(str(e))
            elif reserved:
                try: self.client('relinquish')
                except Exception as e: cleanup_errors.append(str(e))
            if cleanup_errors:
                verdict.update(passed=False, cleanup_errors=cleanup_errors)
            verdict['checks'] = self.checks
            write_json(self.out/'result.json', verdict)
        return verdict


def main():
    budget = Path(os.environ.get('SNIPEXPAND_BUILD_BUDGET', str(Path.home()/'Work/projects/synaxis/scripts/synaxis-build-budget.sh')))
    if budget.is_file() and os.environ.get('SYNAXIS_BUILD_BUDGET_HELD') != '1':
        os.execv(str(budget), [str(budget), '--', sys.executable, str(ROOT/'run.py'), *sys.argv[1:]])
    def terminate(signum, frame):
        # Use the same evidence export and service cleanup as Ctrl+C.
        raise KeyboardInterrupt(f'Received signal {signum}')

    signal.signal(signal.SIGTERM, terminate)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('project', choices=[*DEFAULTS,'all'])
    parser.add_argument('--source',type=Path,help='Source checkout (single project only)')
    parser.add_argument('--image',help='Versioned prepared image name (default: active image)')
    parser.add_argument('--result-file',type=Path,help='Also write the complete summary to this exact path')
    args=parser.parse_args()
    if args.project=='all' and args.source:
        parser.error('--source requires a single project')
    stamp=dt.datetime.now(dt.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    out=ROOT/'artifacts'/stamp
    out.mkdir(mode=0o700)
    lock=os.open(ROOT/'state/run.lock',os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o600)
    try: fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    except BlockingIOError: raise SystemExit('An Oligarchy suite already owns this local VM slot')
    server=None
    db_started=False
    results=[]
    sockdir=Path(os.environ.get('XDG_RUNTIME_DIR',f'/run/user/{os.getuid()}'))/'sx-oligarchy-pg'
    try:
        for port in (24223,42079,39432):
            with socket.socket() as sock:
                if sock.connect_ex(('127.0.0.1',port))==0:
                    raise RuntimeError(f'Port {port} is occupied; refusing to use or stop an unrelated service')
        sockdir.mkdir(mode=0o700,exist_ok=True)
        command([PG/'pg_ctl','-D',ROOT/'state/postgres','-l',out/'postgres.log','-o',f'-h 127.0.0.1 -p 39432 -k {sockdir}','start'],capture_output=True)
        db_started=True
        with (out/'server.log').open('wb') as log:
            server=sp.Popen([sys.executable,str(ROOT/'with-env.py'),'./qemu-server','--name','desktop-tests','--max-jobs','1',
                             '--port','42079','--automation','--data-dir',str(ROOT/'data')],stdout=log,stderr=sp.STDOUT,start_new_session=True)
        deadline=time.monotonic()+30
        while True:
            with socket.socket() as sock:
                if sock.connect_ex(('127.0.0.1',42079))==0: break
            if server.poll() is not None or time.monotonic()>deadline: raise RuntimeError('Oligarchy server failed to start')
            time.sleep(.2)
        for name in DEFAULTS if args.project=='all' else [args.project]:
            source=(args.source or DEFAULTS[name]).resolve()
            spec=importlib.util.spec_from_file_location('suite_adapter',source/'tests/vm/suite.py')
            adapter=importlib.util.module_from_spec(spec)
            spec.loader.exec_module(adapter)
            project_out=out/name
            project_out.mkdir()
            print(f'Running {name} in an Oligarchy VM; evidence: {project_out}',flush=True)
            result=Suite(name,source,project_out,args.image).execute(adapter)
            results.append(result)
            print(json.dumps(result),flush=True)
    finally:
        if server is not None and server.poll() is None:
            server.terminate()
            try: server.wait(timeout=20)
            except sp.TimeoutExpired:
                os.killpg(server.pid,signal.SIGKILL)
                server.wait()
        if db_started:
            command([PG/'pg_ctl','-D',ROOT/'state/postgres','-m','fast','stop'],capture_output=True)
        write_json(out/'summary.json',results)
        if args.result_file:
            write_json(args.result_file,results)
        os.close(lock)
    return 0 if results and all(r['passed'] for r in results) else 1


if __name__=='__main__':
    raise SystemExit(main())
