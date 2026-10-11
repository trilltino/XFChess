"""Run only in a disposable Linux container: exercises real nginx, SQLite and age."""
import http.client
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tarfile
import tempfile
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

OPS = Path("/work")


def run(*args, env=None, input=None, success=True):
    result = subprocess.run(args, env=env, input=input, text=True, capture_output=True, timeout=30)
    if success and result.returncode:
        raise AssertionError(f"{args[0]} failed ({result.returncode}): {result.stderr}\n{result.stdout}")
    if not success and not result.returncode:
        raise AssertionError(f"{args[0]} unexpectedly succeeded")
    return result


def executable(path, content):
    path.write_text(content)
    path.chmod(0o755)


class BackupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.data = self.root / "data"
        self.data.mkdir()
        self.work = self.root / "backups"
        self.remote = self.root / "offsite"
        self.remote.mkdir()
        self.tools = self.root / "bin"
        self.tools.mkdir()
        executable(self.tools / "rclone", "#!/bin/bash\nset -eu\n[[ ${FAIL_UPLOAD:-0} == 0 ]]\ncp -- \"$2\" \"$3\"\n")
        executable(self.tools / "systemctl", "#!/bin/bash\nprintf '%s\\n' \"${SERVICE_STATE:-inactive}\"\n")
        key = self.root / "identity"
        run("age-keygen", "-o", str(key))
        recipient = run("age-keygen", "-y", str(key)).stdout.strip()
        self.env = dict(os.environ, PATH=f"{self.tools}:{os.environ['PATH']}", BACKUP_DATA_DIR=str(self.data),
                        BACKUP_WORK_DIR=str(self.work), BACKUP_REMOTE=str(self.remote),
                        BACKUP_AGE_RECIPIENT=recipient, BACKUP_AGE_IDENTITY=str(key))
        self.db = sqlite3.connect(self.data / "sessions.db")
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("CREATE TABLE games (id INTEGER)")
        self.db.execute("INSERT INTO games VALUES (1)")
        self.db.commit()

    def tearDown(self):
        self.db.close()
        self.temp.cleanup()

    def backup(self, env=None, success=True):
        return run("bash", str(OPS / "backup/backup-db.sh"), env=env or self.env, success=success)

    def restore(self, object, env=None, mode=None, success=True):
        args = ["bash", str(OPS / "backup/restore-db.sh"), "sessions", object]
        if mode:
            args.append(mode)
        return run(*args, env=env or self.env, input="restore\n", success=success)

    def test_live_wal_snapshot_encrypt_check_and_restore(self):
        self.backup()
        object = next(self.remote.iterdir()).name
        self.db.execute("INSERT INTO games VALUES (2)")
        self.db.commit()
        self.restore(object, mode="--check")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM games").fetchone()[0], 2)
        self.restore(object, env=dict(self.env, SERVICE_STATE="active"), success=False)
        self.db.close()
        self.restore(object)
        self.db = sqlite3.connect(self.data / "sessions.db")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM games").fetchone()[0], 1)
        self.assertEqual(sqlite3.connect(next((self.data / "recovery").glob("*.pre-restore-*.db"))).execute("SELECT COUNT(*) FROM games").fetchone()[0], 2)
        self.assertFalse(list(self.work.rglob("*.db")))

    def test_encryption_and_upload_failures_leave_no_plaintext(self):
        self.backup(env=dict(self.env, BACKUP_AGE_RECIPIENT="invalid"), success=False)
        self.assertFalse(list(self.work.rglob("*.db")))
        self.backup(env=dict(self.env, FAIL_UPLOAD="1"), success=False)
        self.assertFalse(list(self.work.rglob("*.db")))

    def test_traversal_corruption_and_retention_are_rejected(self):
        self.restore("../sessions.db.age", success=False)
        self.backup(env=dict(self.env, BACKUP_RETENTION_DAYS="-1"), success=False)
        (self.remote / "sessions-corrupt.db.age").write_text("invalid")
        self.restore("sessions-corrupt.db.age", success=False)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM games").fetchone()[0], 1)


class EchoBackend(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.path == "/ws":
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.end_headers()
            return
        if self.path == "/game/feed":
            self.send_response(209)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Version", '"next"')
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(b"first\n")
            self.wfile.flush()
            time.sleep(1)
            self.wfile.write(b"second\n")
            self.close_connection = True
            return
        body = json.dumps({"path": self.path, "parents": self.headers.get("Parents"),
                           "ip": self.headers.get("X-Real-IP")}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class NginxTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        snippets = Path("/etc/nginx/snippets")
        snippets.mkdir(parents=True, exist_ok=True)
        for source, dest in [("routes.conf", "xfchess-routes.conf"), ("proxy.conf", "xfchess-proxy.conf")]:
            shutil.copy(OPS / "nginx" / source, snippets / dest)
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name)
        cls.root.chmod(0o755)
        (cls.root / "index.html").write_text("frontend")
        cls.config = cls.root / "nginx.conf"
        cls.config.write_text(f"""pid {cls.root}/nginx.pid;
events {{}}
http {{
  access_log off;
  include /work/nginx/xfchess_rate_limit.conf;
  server {{ listen 8088; root {cls.root}; set $xfchess_backend http://127.0.0.1:8090;
    include /etc/nginx/snippets/xfchess-routes.conf; }}
}}
""")
        cls.backend = ThreadingHTTPServer(("127.0.0.1", 8090), EchoBackend)
        threading.Thread(target=cls.backend.serve_forever, daemon=True).start()
        run("nginx", "-t", "-c", str(cls.config))
        cls.nginx = subprocess.Popen(["nginx", "-c", str(cls.config), "-g", "daemon off;"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            import socket
            try:
                with socket.create_connection(("127.0.0.1", 8088), timeout=0.1): break
            except OSError: time.sleep(0.05)
        # Parse all deploy variants with real TLS files, not regex-only checks.
        cert = Path("/etc/letsencrypt/live/example.test")
        cert.mkdir(parents=True, exist_ok=True)
        run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=example.test",
            "-keyout", str(cert / "privkey.pem"), "-out", str(cert / "fullchain.pem"))
        for path in ["nginx/nginx.conf", "nginx/nginx-http.conf", "staging/nginx-staging.conf"]:
            template = (OPS / path).read_text().replace("staging.YOUR_DOMAIN", "example.test").replace("YOUR_DOMAIN", "example.test")
            cfg = cls.root / "variant.conf"
            cfg.write_text("events {}\nhttp { include /work/nginx/xfchess_rate_limit.conf;\n" + template + "\n}")
            run("nginx", "-t", "-c", str(cfg))

    @classmethod
    def tearDownClass(cls):
        cls.nginx.terminate()
        cls.nginx.wait(timeout=5)
        cls.backend.shutdown()
        cls.temp.cleanup()

    def test_public_routes_and_resume_headers(self):
        for path in ["/api/tournament/1", "/p2p/rooms", "/friends/list", "/presence/list", "/offline-tournaments/1"]:
            connection = http.client.HTTPConnection("127.0.0.1", 8088, timeout=3)
            connection.request("GET", path, headers={"Parents": '"checkpoint"'})
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            result = json.loads(response.read())
            self.assertEqual(result["path"], path)
            self.assertEqual(result["parents"], '"checkpoint"')
            self.assertEqual(result["ip"], "127.0.0.1")
            connection.close()

    def test_private_routes_never_fall_back_to_spa(self):
        for path in ["/admin", "/admin/metrics/summary", "/metrics"]:
            connection = http.client.HTTPConnection("127.0.0.1", 8088, timeout=3)
            connection.request("GET", path)
            with self.assertRaises(http.client.RemoteDisconnected):
                connection.getresponse()
            connection.close()

    def test_stream_and_websocket_are_delivered(self):
        connection = http.client.HTTPConnection("127.0.0.1", 8088, timeout=3)
        start = time.monotonic()
        connection.request("GET", "/game/feed")
        response = connection.getresponse()
        self.assertEqual(response.status, 209)
        self.assertEqual(response.getheader("Version"), '"next"')
        self.assertEqual(response.readline(), b"first\n")
        self.assertLess(time.monotonic() - start, 0.8)
        response.read()
        connection.close()

        connection = http.client.HTTPConnection("127.0.0.1", 8088, timeout=3)
        connection.request("GET", "/ws", headers={"Upgrade": "websocket", "Connection": "Upgrade"})
        response = connection.getresponse()
        self.assertEqual(response.status, 101)
        self.assertEqual(response.getheader("Upgrade"), "websocket")
        connection.close()

class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.fixture = Path(self.temp.name)
        self.root = Path("/opt/xfchess")
        if self.root.exists():
            shutil.rmtree(self.root)
        self.root.mkdir(parents=True)
        self.root.chmod(0o755)
        run("bash", "-c", "id xfchess >/dev/null 2>&1 || useradd --system xfchess")
        self.tools = self.fixture / "bin"
        self.tools.mkdir()
        self.fixture.chmod(0o755)
        executable(self.tools / "cargo", """#!/bin/bash
set -eu
[[ ${FAIL_BUILD:-0} == 0 ]]
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\\necho backend\\n' >"$CARGO_TARGET_DIR/release/signing-server-http"
chmod 755 "$CARGO_TARGET_DIR/release/signing-server-http"
""")
        executable(self.tools / "systemctl", "#!/bin/sh\nexit 0\n")
        executable(self.tools / "sleep", "#!/bin/sh\nexit 0\n")
        executable(self.tools / "nginx", "#!/bin/sh\n! grep -q '^broken' /etc/nginx/sites-available/xfchess\n")
        executable(self.tools / "curl", """#!/bin/bash
set -eu
sha=$(cat /opt/xfchess/current/commit)
[[ ${FAIL_HEALTH:-none} != "$sha" ]] || exit 22
if [[ ${*: -1} == */health ]]; then printf '{"status":"ok","git_sha":"%s"}' "$sha"; fi
""")
        self.env = dict(os.environ, PATH=f"{self.tools}:{os.environ['PATH']}")
        (self.root / ".env").write_text("SESSION_DB_URL=sqlite:///opt/xfchess/data/sessions.db?mode=rwc\n")
        (self.root / "data").mkdir()
        with sqlite3.connect(self.root / "data/sessions.db") as db:
            db.execute("CREATE TABLE games(id INTEGER)")
            db.execute("INSERT INTO games VALUES (1)")
        (self.root / "web").mkdir()
        (self.root / "web/index.html").write_text("legacy website")
        executable(self.root / "signing-server-http", "#!/bin/sh\nexit 0\n")
        for directory in ["/etc/nginx/sites-available", "/etc/nginx/sites-enabled", "/etc/nginx/conf.d", "/etc/nginx/snippets", "/etc/systemd/system", "/etc/letsencrypt/live/example.test"]:
            Path(directory).mkdir(parents=True, exist_ok=True)
        (Path("/etc/letsencrypt/live/example.test") / "fullchain.pem").touch()
        Path("/etc/nginx/sites-available/xfchess").write_text("legacy config")
        link = Path("/etc/nginx/sites-enabled/xfchess")
        if link.is_symlink():
            link.unlink()
        link.symlink_to("/etc/nginx/sites-available/xfchess")

    def tearDown(self):
        self.temp.cleanup()

    def deploy(self, sha, success=True, broken=False, env=None):
        release = f"{sha}-test"
        incoming = self.root / "incoming" / release
        incoming.mkdir(parents=True)
        src = self.fixture / release
        shutil.copytree(OPS, src / "ops")
        if broken:
            with (src / "ops/nginx/nginx.conf").open("a") as config:
                config.write("\nbroken\n")
        with tarfile.open(incoming / "source.tar.gz", "w:gz") as archive:
            archive.add(src / "ops", arcname="ops")
        web = self.fixture / f"web-{release}"
        web.mkdir()
        (web / "index.html").write_text(sha)
        with tarfile.open(incoming / "web.tar.gz", "w:gz") as archive:
            archive.add(web / "index.html", arcname="index.html")
        run("bash", str(OPS / "scripts/deploy.sh"), release, sha, "example.test", env=env or self.env, success=success)
        return release

    def test_deploy_and_rollback_preserve_data_and_switch_the_website(self):
        first = self.deploy("a" * 40)
        second = self.deploy("b" * 40)
        with sqlite3.connect(self.root / "data/sessions.db") as db:
            db.execute("INSERT INTO games VALUES (2)")
        run("bash", str(OPS / "scripts/rollback.sh"), env=self.env)
        self.assertEqual((self.root / "web/index.html").read_text(), "a" * 40)
        self.assertEqual((self.root / "current").resolve().name, first)
        self.assertEqual((self.root / "previous").resolve().name, second)
        with sqlite3.connect(self.root / "data/sessions.db") as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM games").fetchone()[0], 2)
        self.assertTrue((self.root / "backups" / f"pre-{first}" / "sessions.db").is_file())

    def test_failed_proxy_validation_and_health_restore_previous_release(self):
        first = self.deploy("a" * 40)
        self.deploy("b" * 40, success=False, broken=True)
        self.assertEqual((self.root / "current").resolve().name, first)
        self.deploy("c" * 40, success=False, env=dict(self.env, FAIL_HEALTH="c" * 40))
        self.assertEqual((self.root / "current").resolve().name, first)
        self.assertEqual((self.root / "web/index.html").read_text(), "a" * 40)

    def test_legacy_rollback_keeps_recovery_tools_and_preserves_old_ops(self):
        (self.root / "ops").mkdir()
        (self.root / "ops/old-script").write_text("operator tooling")
        first = self.deploy("a" * 40)
        run("bash", str(OPS / "scripts/rollback.sh"), env=self.env)
        self.assertEqual((self.root / "web/index.html").read_text(), "legacy website")
        self.assertTrue((self.root / "ops/backup/restore-db.sh").is_file())
        self.assertEqual((self.root / f"ops.before-{first}/old-script").read_text(), "operator tooling")

    def test_release_lock_blocks_a_second_deployment(self):
        import fcntl
        lock = (self.root / ".release.lock").open("w")
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.deploy("a" * 40, success=False)
            self.assertFalse((self.root / "current").exists())
        finally:
            lock.close()

if __name__ == "__main__":
    if not Path("/.dockerenv").exists() or os.geteuid() != 0:
        raise SystemExit("Run in a disposable root Docker container with ops mounted read-only at /work.")
    for script in OPS.rglob("*.sh"):
        run("bash", "-n", str(script))
    unittest.main(verbosity=2)
