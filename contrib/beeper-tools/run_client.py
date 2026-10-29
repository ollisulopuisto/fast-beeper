#!/usr/bin/env python3
"""Run the compiled Beeper client locally. Credentials stay in local child environment."""
import argparse
import getpass
import json
import os
import pathlib
import subprocess
import sys
import socket
import urllib.error
import urllib.request
import urllib.parse


LOOPBACK = ('localhost', '127.0.0.1', '::1')

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Endpoint redirected. No token was forwarded. Check the existing endpoint address.')


def validate_endpoint(base):
    parsed = urllib.parse.urlsplit(base)
    if (not parsed.hostname or parsed.username or parsed.password
            or parsed.scheme not in ('https', 'http') or parsed.query or parsed.fragment
            or parsed.path not in ('', '/')):
        raise ValueError('Use the existing API server root URL, without credentials, path, query or fragment.')
    if parsed.scheme == 'http' and parsed.hostname not in LOOPBACK:
        raise ValueError('Remote HTTP is refused. Use HTTPS or loopback.')
    # Force validation of the optional port before prompting for a token.
    _ = parsed.port
    return parsed


def connection_hint(error):
    if isinstance(error, urllib.error.URLError):
        error = error.reason
    if isinstance(error, ConnectionRefusedError):
        return ('Connection refused: nothing accepted the connection at this address. '
                'For Desktop, keep Beeper Desktop running and check Settings > Integrations > API. '
                'For an existing headless Server, check its configured address and running state. '
                'This launcher does not install or start a backend. Do not create another server or token to fix a refused connection.')
    if isinstance(error, (TimeoutError, socket.timeout)):
        return 'Connection timed out. Check the existing endpoint and local network/proxy settings; no new server or token was created.'
    return 'Connection failed before a usable HTTP response. Check the existing endpoint and local TLS/network settings. No token or server response is printed.'


def preflight(base, token=None):
    parsed = validate_endpoint(base)
    handlers = [NoRedirect()]
    if parsed.hostname in LOOPBACK:
        handlers.append(urllib.request.ProxyHandler({}))
    opener = urllib.request.build_opener(*handlers)
    headers = {'Authorization': 'Bearer ' + token} if token else {}
    request = urllib.request.Request(base.rstrip('/') + '/v1/accounts', headers=headers)
    try:
        with opener.open(request, timeout=5) as response:
            if response.status != 200:
                raise ValueError('Unexpected HTTP status from accounts endpoint: %s' % response.status)
            # Validate the expected API shape in bounded transient memory. Never
            # print/store/log the private payload or treat a200loginpage as ready.
            body = response.read(16 * 1024 * 1024 + 1)
            if len(body) > 16 * 1024 * 1024:
                raise ValueError('Accounts response exceeded the safe size limit.')
            try:
                accounts = json.loads(body)
            except (ValueError, UnicodeError):
                raise ValueError('Endpoint did not return a valid accounts API response; no private payload was printed.') from None
            if not isinstance(accounts, list) or any(not isinstance(account, dict) for account in accounts):
                raise ValueError('Endpoint did not return an accounts array; check the existing API root.')
    except urllib.error.HTTPError as error:
        error.close()
        if error.code == 401 and token is None:
            return  # Reachable authenticated API. Prompt for token next.
        if error.code in (401, 403):
            raise ValueError('HTTP %s: API is reachable, but access was rejected. Check the token locally in Beeper Settings > Integrations; never paste it into chat.' % error.code) from None
        if 300 <= error.code < 400:
            raise ValueError('Redirect refused; no token forwarded. Check the existing API server root URL.') from None
        raise ValueError('HTTP %s: server is reachable but /v1/accounts was not accepted. Check the existing API address/version.' % error.code) from None
    except (urllib.error.URLError, OSError) as error:
        raise ValueError(connection_hint(error)) from None


def loopback_child_environment(env, parsed):
    env = env.copy()
    if parsed.hostname in LOOPBACK:
        # reqwest and platform tools may inspect either spelling. Preserve all
        # existing exclusions and add only the explicit loopback destination.
        for key in ('NO_PROXY', 'no_proxy'):
            env[key] = ','.join(filter(None, (env.get(key, ''), 'localhost,127.0.0.1,::1')))
    return env


def resolve_beeper_target(name):
    if not name or len(name) > 80 or not all(ch.isalnum() or ch in '-_.' for ch in name):
        raise ValueError('Use an existing explicit Beeper CLI target name.')
    # Inspect only the named existing profile. Never setup/start/select a target.
    # CLI output may contain its token; keep it private and never print stderr.
    try:
        result = subprocess.run(['beeper','targets','show',name,'--json'], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=10, check=False)
    except (FileNotFoundError, subprocess.TimeoutExpired):
        # Shell aliases/functions are not executables inherited by Python; the
        # official CLI's exact named profile is a local source of truth too.
        # Read only this existing target, never a Desktop/default fallback.
        root = pathlib.Path(os.environ.get('BEEPER_CLI_CONFIG_DIR') or pathlib.Path.home() / '.beeper')
        try:
            path = root / 'targets' / (name + '.json')
            with path.open('rb') as stream:
                body = stream.read(1024 * 1024 + 1)
            if len(body) > 1024 * 1024:
                raise ValueError()
            target = json.loads(body)
            if target.get('id') != name:
                raise ValueError()
            base = target['baseURL']
            validate_endpoint(base)
            token = (target.get('auth') or {}).get('accessToken')
            if token is not None and (not isinstance(token, str) or not token or len(token) > 16384 or any(ch in token for ch in '\r\n')):
                raise ValueError()
            return base, token
        except (OSError, ValueError, TypeError, KeyError, AttributeError):
            raise ValueError('The CLI executable was unavailable or timed out, and its existing named target profile could not be read safely. No target, default or Server was changed; no private output was printed.') from None
    if result.returncode or len(result.stdout) > 1024 * 1024:
        raise ValueError('Existing target lookup failed. No target was created, changed or started.')
    try:
        envelope = json.loads(result.stdout)
        target = envelope['data']
        if envelope.get('success') is not True or target.get('id') != name:
            raise ValueError()
        base = target['baseURL']
        validate_endpoint(base)
        token = (target.get('auth') or {}).get('accessToken')
        if token is not None and (not isinstance(token,str) or not token or len(token)>16384 or any(ch in token for ch in '\r\n')):
            raise ValueError()
        return base, token
    except (ValueError, TypeError, KeyError, AttributeError):
        raise ValueError('Existing target data was invalid. No private target output was printed.') from None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    route = parser.add_mutually_exclusive_group()
    route.add_argument('--base-url', help='Existing API server root; Desktop defaults to loopback port23373')
    route.add_argument('--beeper-target', help='Read one existing Beeper CLI target, without setup/start/default changes')
    parser.add_argument('--sign-in', action='store_true', help='Open in-app sign-in for an installed local Server; no terminal token prompt')
    parser.add_argument('--check-only', action='store_true', help='Check transport only, without prompting for a token or launching the client')
    parser.add_argument('--allow-writes', action='store_true', help='Enable deliberate sends, typing and mark-read changes')
    parser.add_argument('--binary', help='Existing local ZapFast binary, instead of cargo run')
    args = parser.parse_args()
    target_token = None
    if args.beeper_target:
        try:
            args.base_url, target_token = resolve_beeper_target(args.beeper_target)
        except ValueError as error:
            print(str(error), file=sys.stderr)
            return 1
    args.base_url = args.base_url or 'http://127.0.0.1:23373'
    try:
        parsed = validate_endpoint(args.base_url)
        if not args.sign_in:
            preflight(args.base_url)
        elif parsed.hostname not in LOOPBACK:
            raise ValueError('In-app sign-in requires a local loopback Server.')
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    if args.check_only:
        print('Accounts endpoint reachable. Authentication and real-account compatibility remain separate checks.')
        return 0
    # A selected target must never inherit a Desktop token from the shell.
    token = (target_token if args.beeper_target else os.environ.get('BEEPER_ACCESS_TOKEN')) or ('' if args.sign_in else getpass.getpass('Beeper API token (local terminal only): '))
    if not token and not args.sign_in:
        parser.error('Token is empty')
    try:
        if not args.sign_in:
            preflight(args.base_url, token)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    env = loopback_child_environment(os.environ, parsed)
    env.update(ZAPFAST_BACKEND='beeper', BEEPER_DESKTOP_BASE_URL=args.base_url,
               BEEPER_ACCESS_TOKEN=token, BEEPER_SIGN_IN='1' if args.sign_in else '0', BEEPER_READONLY='0' if args.allow_writes else '1')
    # Keep the prototype separate from any installed stock WhatsApp ZapFast profile.
    state = pathlib.Path.home() / '.zapfast-beeper-prototype'
    env['ZAPFAST_PROFILE_ROOT'] = str(state)
    for kind in ('config', 'data', 'cache'):
        directory = state / kind
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        env['XDG_%s_HOME' % kind.upper()] = str(directory)
    print('Write actions ENABLED.' if args.allow_writes else 'Read-only mode: no sends, typing or read-state changes.')
    print('Using an existing endpoint only. No accounts will be linked or migrated by this launcher.')
    command = [os.path.abspath(args.binary)] if args.binary else ['cargo', 'run', '--locked', '--bin', 'zapfast']
    root = pathlib.Path(__file__).resolve().parents[2]
    try:
        return subprocess.call(command, cwd=root, env=env)
    except FileNotFoundError:
        print('Cargo or binary not found. Build prerequisites are in README.md.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
