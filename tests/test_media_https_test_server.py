import json
import os
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

from scripts.media_https_test_server import (
    Handler, MediaHTTPServer, client_timeout_for_address, validate_request,
)

TOKEN = "a" * 64
REQUEST_ID = "0123456789abcdef0123456789abcdef"
NOW = 1_783_586_658_000


def body(**overrides):
    value = {
        "id": REQUEST_ID,
        "cmd": "pause",
        "issued_at_ms": NOW,
        "expires_at_ms": NOW + 5000,
    }
    value.update(overrides)
    return json.dumps(value, separators=(",", ":")).encode()


class MediaHttpsContractTest(unittest.TestCase):
    def test_client_timeout_only_for_non_loopback_bind_addresses(self):
        self.assertIsNone(client_timeout_for_address("127.0.0.1"))
        self.assertIsNone(client_timeout_for_address("::1"))
        self.assertEqual(client_timeout_for_address("192.0.2.10"), 5)
        self.assertEqual(client_timeout_for_address("0.0.0.0"), 5)

    def validate(self, payload=None, headers=None, now=NOW, seen=None):
        return validate_request(
            "/v1/media",
            headers if headers is not None else {
                "Authorization": f"Bearer {TOKEN}", "Content-Type": "application/json"
            },
            payload if payload is not None else body(),
            TOKEN,
            now,
            seen if seen is not None else {},
        )

    def test_accepts_valid_media_request_without_execution(self):
        result = self.validate()
        self.assertEqual(result.status, 202)
        self.assertEqual(result.body["status"], "accepted")
        self.assertEqual(result.body["id"], REQUEST_ID)

    def test_rejects_wrong_path_and_token(self):
        self.assertEqual(self.validate_path("/other").status, 404)
        self.assertEqual(
            self.validate(headers={"Authorization": "Bearer " + "b" * 64,
                              "Content-Type": "application/json"}).status,
            401,
        )

    def validate_path(self, path):
        return validate_request(
            path, {"Authorization": f"Bearer {TOKEN}"}, body(), TOKEN, NOW, {}
        )

    def test_rejects_unknown_fields_and_commands(self):
        for updates in (
            {"v": 1},
            {"cmd": "shutdown"},
            {"extra": True},
            {"cmd": 4},
        ):
            with self.subTest(updates=updates):
                self.assertEqual(self.validate(body(**updates)).status, 400)

    def test_rejects_duplicate_json_keys_and_oversize(self):
        duplicate = (f'{{"id":"{REQUEST_ID}","id":"{REQUEST_ID}"}}').encode()
        self.assertEqual(self.validate(duplicate).status, 400)
        self.assertEqual(self.validate(b" " * 513).status, 413)

    def test_rejects_missing_token_and_bad_expiry(self):
        self.assertEqual(self.validate(headers={"Content-Type": "application/json"}).status, 401)
        self.assertEqual(self.validate(headers={"Authorization": f"Bearer {TOKEN}"}).status, 400)
        self.assertEqual(self.validate(body(expires_at_ms=NOW + 5001)).status, 400)

    def test_expiry_and_future_skew(self):
        self.assertEqual(self.validate(now=NOW + 5000).body["status"], "expired")
        self.assertEqual(
            self.validate(body(issued_at_ms=NOW + 5000,
                               expires_at_ms=NOW + 10000)).status,
            202,
        )
        skewed = self.validate(body(issued_at_ms=NOW + 5001,
                                    expires_at_ms=NOW + 10001))
        self.assertEqual(skewed.status, 400)
        self.assertEqual(skewed.body["status"], "clock_skew")

    def test_rejects_duplicate_ids_and_full_live_cache(self):
        seen = {}
        self.assertEqual(self.validate(seen=seen).status, 202)
        self.assertEqual(self.validate(seen=seen).status, 409)
        full = {f"{n:032x}": NOW + 4000 for n in range(128)}
        self.assertEqual(self.validate(seen=full).status, 429)

    def test_expired_duplicate_entries_are_removed(self):
        seen = {"deadbeef" * 4: NOW}
        self.assertEqual(self.validate(now=NOW + 1, seen=seen).status, 202)
        self.assertEqual(len(seen), 1)


class MediaHttpsServerIntegrationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        openssl = shutil.which("openssl")
        if not openssl:
            raise unittest.SkipTest("openssl CLI unavailable for disposable TLS fixture")
        cls.temp = tempfile.TemporaryDirectory()
        cls.directory = Path(cls.temp.name)
        cls.cert = cls.directory / "server.pem"
        cls.key = cls.directory / "server-key.pem"
        subprocess.run(
            [openssl, "req", "-x509", "-newkey", "rsa:2048", "-nodes",
             "-keyout", str(cls.key), "-out", str(cls.cert), "-days", "1",
             "-subj", "/CN=localhost",
             "-addext", "subjectAltName=DNS:localhost",
             "-addext", "basicConstraints=critical,CA:TRUE",
             "-addext", "keyUsage=critical,digitalSignature,keyCertSign"],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            cls.port = sock.getsockname()[1]
        cls.token = "c" * 64
        env = os.environ.copy()
        env["MEDIA_TEST_TOKEN"] = cls.token
        server_script = Path(__file__).resolve().parents[1] / "scripts" / "media_https_test_server.py"
        cls.process = subprocess.Popen(
            [sys.executable, str(server_script), "--host", "127.0.0.1",
             "--port", str(cls.port), "--cert", str(cls.cert), "--key", str(cls.key)],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        cls.context = ssl.create_default_context(cafile=str(cls.cert))
        deadline = time.monotonic() + 5
        while True:
            if cls.process.poll() is not None:
                raise RuntimeError("HTTPS fixture exited during startup")
            try:
                urllib.request.urlopen(
                    f"https://localhost:{cls.port}/", context=cls.context, timeout=1
                )
            except urllib.error.HTTPError as error:
                if error.code == 405:
                    error.close()
                    break
            except (OSError, urllib.error.URLError):
                pass
            if time.monotonic() >= deadline:
                raise RuntimeError("HTTPS fixture failed to start")
            time.sleep(0.02)

    @classmethod
    def tearDownClass(cls):
        if hasattr(cls, "process"):
            cls.process.terminate()
            output, _ = cls.process.communicate(timeout=5)
            if hasattr(cls, "token"):
                assert cls.token not in output
        if hasattr(cls, "temp"):
            cls.temp.cleanup()

    def post(self, payload, token=None):
        request = urllib.request.Request(
            f"https://localhost:{self.port}/v1/media", data=payload,
            headers={"Content-Type": "application/json",
                     "Authorization": f"Bearer {token or self.token}"}, method="POST",
        )
        return urllib.request.urlopen(request, context=self.context, timeout=3)

    def test_verified_tls_accepts_and_deduplicates_without_execution(self):
        now = int(time.time() * 1000)
        payload = json.dumps({
            "id": "f" * 32, "cmd": "pause",
            "issued_at_ms": now, "expires_at_ms": now + 5000,
        }, separators=(",", ":")).encode()
        with self.post(payload) as response:
            self.assertEqual(response.status, 202)
            result = json.loads(response.read())
            self.assertEqual(result, {"id": "f" * 32, "status": "accepted"})
        with self.assertRaises(urllib.error.HTTPError) as duplicate:
            self.post(payload)
        self.assertEqual(duplicate.exception.code, 409)
        duplicate.exception.close()
        with self.assertRaises(urllib.error.HTTPError) as unauthorized:
            self.post(payload, "0" * 64)
        self.assertEqual(unauthorized.exception.code, 401)
        unauthorized.exception.close()

    def test_non_post_method_is_rejected(self):
        request = urllib.request.Request(f"https://localhost:{self.port}/v1/media")
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(request, context=self.context, timeout=3)
        self.assertEqual(error.exception.code, 405)
        error.exception.close()

    def test_stalled_tls_handshake_and_request_reads_have_deadlines(self):
        server = MediaHTTPServer(("127.0.0.1", 0), Handler)
        server.expected_token = self.token
        server.seen = {}
        server.client_timeout = 0.3
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.minimum_version = ssl.TLSVersion.TLSv1_2
        tls.load_cert_chain(self.cert, self.key)
        server.socket = tls.wrap_socket(
            server.socket, server_side=True, do_handshake_on_connect=False
        )
        thread = threading.Thread(
            target=server.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True
        )
        thread.start()
        base_url = f"https://localhost:{server.server_address[1]}"
        try:
            stalled_handshake = socket.create_connection(
                ("127.0.0.1", server.server_address[1]), timeout=2
            )
            start = time.monotonic()
            self.assert_method_not_allowed(base_url)
            self.assertLess(time.monotonic() - start, 2)
            stalled_handshake.close()

            stalled_request = self.context.wrap_socket(
                socket.create_connection(("127.0.0.1", server.server_address[1]), timeout=2),
                server_hostname="localhost",
            )
            stalled_request.sendall(b"GET / HTTP/1.1\r\nHost:")
            start = time.monotonic()
            self.assert_method_not_allowed(base_url)
            self.assertLess(time.monotonic() - start, 2)
            stalled_request.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def assert_method_not_allowed(self, base_url):
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(base_url + "/", context=self.context, timeout=3)
        self.assertEqual(error.exception.code, 405)
        error.exception.close()

    def test_wrong_hostname_fails_tls(self):
        request = urllib.request.Request(
            f"https://127.0.0.1:{self.port}/v1/media", data=body(),
            headers={"Content-Type": "application/json",
                     "Authorization": f"Bearer {self.token}"}, method="POST",
        )
        with self.assertRaises(urllib.error.URLError):
            urllib.request.urlopen(request, context=self.context, timeout=3)


if __name__ == "__main__":
    unittest.main()
