import contextlib
import http.server
import importlib.util
import io
import os
import pathlib
import socket
import sys
import tempfile
import threading
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('launcher', pathlib.Path(__file__).with_name('run_client.py'))
launcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launcher)

class Handler(http.server.BaseHTTPRequestHandler):
    status = 401
    requests = []
    def log_message(self, *args): pass
    def do_GET(self):
        self.requests.append((self.path, self.headers.get('Authorization')))
        status = self.status
        if status == 'auth': status = 200 if self.headers.get('Authorization') == 'Bearer local-only' else 401
        self.send_response(status)
        if status == 302: self.send_header('Location', 'http://127.0.0.1:1/no-forward')
        self.end_headers()
        if status == 200: self.wfile.write(b'[{"private":"must not print"}]')

class LauncherTests(unittest.TestCase):
    def setUp(self):
        Handler.requests = []
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1',0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.start()
        self.base = 'http://127.0.0.1:%s' % self.server.server_port
    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()
    def test_auth_is_separate_from_transport_and_never_prints_body(self):
        Handler.status = 'auth'
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            launcher.preflight(self.base)
            launcher.preflight(self.base, 'local-only')
        self.assertEqual(output.getvalue(), '')
        self.assertEqual(Handler.requests, [('/v1/accounts',None),('/v1/accounts','Bearer local-only')])
    def test_success_status_with_non_api_payload_is_not_accepted(self):
        class Response(io.BytesIO):
            status = 200
            def __enter__(self):return self
            def __exit__(self,*args):self.close()
        opener = mock.Mock()
        for payload in (b'<html>login page</html>', b'{"private":"not an accounts array"}', b' ' * (16*1024*1024+2)):
            opener.open.return_value = Response(payload)
            with mock.patch.object(launcher.urllib.request,'build_opener',return_value=opener), self.assertRaises(ValueError):
                launcher.preflight(self.base,'fixture')

    def test_bad_token_is_http_not_transport(self):
        Handler.status = 401
        with self.assertRaisesRegex(ValueError,'HTTP 401: API is reachable'):
            launcher.preflight(self.base,'wrong-local-token')
    def test_redirect_never_forwards_token(self):
        Handler.status = 302
        with self.assertRaisesRegex(ValueError,'redirected|Redirect'):
            launcher.preflight(self.base,'local-only')
        self.assertEqual(len(Handler.requests),1)
    def test_refusal_explains_backend_without_token_leak(self):
        self.server.shutdown(); self.server.server_close()
        with self.assertRaisesRegex(ValueError,'Connection refused'):
            launcher.preflight(self.base,'secret-never-printed')
    def test_loopback_ignores_proxy_environment(self):
        Handler.status = 401
        with mock.patch.dict(os.environ,{'http_proxy':'http://127.0.0.1:1','HTTP_PROXY':'http://127.0.0.1:1','NO_PROXY':'','no_proxy':''}):
            launcher.preflight(self.base)
    def test_refuses_bad_endpoint_before_credentials(self):
        for base in ['http://user:secret@127.0.0.1', 'http://192.168.1.2', 'http://127.0.0.1/v1/accounts','http://127.0.0.1?token=bad']:
            with self.assertRaises(ValueError): launcher.validate_endpoint(base)
    def test_child_retains_exclusions_and_adds_loopback(self):
        env=launcher.loopback_child_environment({'NO_PROXY':'internal.example'},launcher.validate_endpoint(self.base))
        self.assertIn('internal.example',env['NO_PROXY']); self.assertIn('127.0.0.1',env['no_proxy'])
    def test_target_resolution_is_exact_and_read_only(self):
        import json
        data={'success':True,'data':{'id':'zapfast-headless','baseURL':self.base,'auth':{'accessToken':'local-only'}}}
        result=mock.Mock(returncode=0,stdout=json.dumps(data).encode())
        with mock.patch.object(launcher.subprocess,'run',return_value=result) as run:
            self.assertEqual(launcher.resolve_beeper_target('zapfast-headless'),(self.base,'local-only'))
            self.assertEqual(run.call_args.args[0],['beeper','targets','show','zapfast-headless','--json'])
        data['data']['id']='other'
        result.stdout=json.dumps(data).encode()
        with mock.patch.object(launcher.subprocess,'run',return_value=result), self.assertRaisesRegex(ValueError,'private target output'):
            launcher.resolve_beeper_target('zapfast-headless')
    def test_explicit_target_does_not_use_desktop_environment_token(self):
        Handler.status='auth'
        with mock.patch('sys.argv',['run_client.py','--beeper-target','zapfast-headless','--binary','/tmp/fixture-binary']), mock.patch.object(launcher,'resolve_beeper_target',return_value=(self.base,'local-only')), mock.patch.dict(os.environ,{'BEEPER_ACCESS_TOKEN':'wrong-desktop'}), mock.patch.object(launcher.subprocess,'call',return_value=0) as call, tempfile.TemporaryDirectory() as home, mock.patch.object(launcher.pathlib.Path,'home',return_value=pathlib.Path(home)), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(launcher.main(),0)
            self.assertEqual(call.call_args.kwargs['env']['BEEPER_ACCESS_TOKEN'],'local-only')
            self.assertEqual(call.call_args.kwargs['env']['BEEPER_DESKTOP_BASE_URL'],self.base)
            self.assertEqual(call.call_args.kwargs['env']['BEEPER_READONLY'],'1')

    def test_unreachable_endpoint_never_prompts_or_launches(self):
        self.server.shutdown(); self.server.server_close()
        with mock.patch('sys.argv',['run_client.py','--base-url',self.base]), mock.patch.object(launcher.getpass,'getpass') as prompt, mock.patch.object(launcher.subprocess,'call') as launch, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(launcher.main(),1)
            prompt.assert_not_called(); launch.assert_not_called()


class SignInLauncherTests(unittest.TestCase):
    def test_signin_flag_reaches_gui_without_token_prompt_or_accounts_preflight(self):
        with mock.patch.object(sys, 'argv', ['launcher.py', '--base-url', 'http://127.0.0.1:23374', '--sign-in', '--binary', '/tmp/synthetic-binary']), \
             mock.patch.object(launcher, 'preflight') as preflight, \
             mock.patch.object(launcher.getpass, 'getpass', side_effect=AssertionError('terminal prompt')), \
             mock.patch.object(launcher.subprocess, 'call', return_value=0) as launch, \
             mock.patch.object(launcher.pathlib.Path, 'mkdir'):
            self.assertEqual(launcher.main(), 0)
            preflight.assert_not_called()
            self.assertEqual(launch.call_args.kwargs['env']['BEEPER_SIGN_IN'], '1')

class TargetFileFallbackTests(unittest.TestCase):
    def test_shell_only_cli_uses_exact_existing_named_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            targets = pathlib.Path(tmp) / 'targets'
            targets.mkdir()
            (targets / 'server.json').write_text('{"id":"server","baseURL":"http://127.0.0.1:23374","auth":{"accessToken":"synthetic-server"}}')
            with mock.patch.dict(os.environ, {'BEEPER_CLI_CONFIG_DIR':tmp,'BEEPER_ACCESS_TOKEN':'wrong-desktop'}), mock.patch.object(launcher.subprocess,'run',side_effect=FileNotFoundError):
                self.assertEqual(launcher.resolve_beeper_target('server'),('http://127.0.0.1:23374','synthetic-server'))
    def test_timeout_uses_same_existing_profile_and_validates_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            targets = pathlib.Path(tmp) / 'targets'; targets.mkdir()
            (targets / 'server.json').write_text('{"id":"desktop","baseURL":"http://127.0.0.1:23374","auth":{"accessToken":"private-not-output"}}')
            with mock.patch.dict(os.environ, {'BEEPER_CLI_CONFIG_DIR':tmp}), mock.patch.object(launcher.subprocess,'run',side_effect=launcher.subprocess.TimeoutExpired('beeper',10)):
                with self.assertRaisesRegex(ValueError, 'could not be read safely') as error:
                    launcher.resolve_beeper_target('server')
                self.assertNotIn('private-not-output',str(error.exception))
    def test_missing_profile_never_creates_target(self):
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.dict(os.environ, {'BEEPER_CLI_CONFIG_DIR':tmp}), mock.patch.object(launcher.subprocess,'run',side_effect=FileNotFoundError):
                with self.assertRaises(ValueError): launcher.resolve_beeper_target('server')
            self.assertEqual(list(pathlib.Path(tmp).iterdir()),[])

if __name__ == '__main__': unittest.main()
