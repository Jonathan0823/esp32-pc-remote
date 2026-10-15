#!/usr/bin/env python3
"""Non-actioning HTTPS fixture for the ESP media-control protocol."""

import argparse
import hmac
import json
import os
import re
import ssl
import time
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, HTTPServer

MAX_BODY = 512
MAX_IDS = 128
LIFETIME_MS = 5000
MAX_FUTURE_SKEW_MS = 1000
COMMANDS = {"play", "pause", "next", "previous", "mute", "unmute"}
ID_RE = re.compile(r"^[0-9a-f]{32}$")
TOKEN_RE = re.compile(r"^[0-9a-f]{64}$")
FIELDS = {"id", "cmd", "issued_at_ms", "expires_at_ms"}
MAX_TIMESTAMP = (1 << 63) - 1


@dataclass
class Result:
    status: int
    body: dict
    command: str = ""


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate json key")
        result[key] = value
    return result


def _response(status, request_id="", message=""):
    body = {"status": message}
    if request_id:
        body["id"] = request_id
    return Result(status, body)


def validate_request(path, headers, body, expected_token, now_ms, seen):
    """Validate one request; mutate seen only after complete validation."""
    if path != "/v1/media":
        return _response(404, message="not_found")

    content_type = headers.get("Content-Type", "").split(";", 1)[0].strip().lower()
    if content_type != "application/json":
        return _response(400, message="content_type")

    authorization = headers.get("Authorization", "")
    prefix = "Bearer "
    supplied = authorization[len(prefix):] if authorization.startswith(prefix) else ""
    if not supplied or not hmac.compare_digest(supplied, expected_token):
        return _response(401, message="unauthorized")

    if len(body) > MAX_BODY:
        return _response(413, message="too_large")
    try:
        request = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
        return _response(400, message="invalid_json")
    if not isinstance(request, dict) or set(request) != FIELDS:
        return _response(400, message="invalid_fields")

    request_id = request["id"]
    command = request["cmd"]
    issued = request["issued_at_ms"]
    expires = request["expires_at_ms"]
    if not isinstance(request_id, str) or not ID_RE.fullmatch(request_id):
        return _response(400, message="invalid_id")
    if not isinstance(command, str) or command not in COMMANDS:
        return _response(400, message="invalid_command")
    if (type(issued) is not int or type(expires) is not int or
            issued < 0 or expires < 0 or issued > MAX_TIMESTAMP or expires > MAX_TIMESTAMP or
            issued > MAX_TIMESTAMP - LIFETIME_MS or expires != issued + LIFETIME_MS):
        return _response(400, message="invalid_time")
    if issued > now_ms and issued - now_ms > MAX_FUTURE_SKEW_MS:
        return _response(400, message="clock_skew")
    if now_ms >= expires:
        return _response(410, request_id, "expired")

    for old_id, old_expiry in list(seen.items()):
        if old_expiry <= now_ms:
            del seen[old_id]
    if request_id in seen:
        return _response(409, request_id, "duplicate")
    if len(seen) >= MAX_IDS:
        return _response(429, request_id, "busy")

    seen[request_id] = expires
    return Result(202, {"id": request_id, "status": "accepted"}, command)


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    server_version = "MediaContractFixture/1"

    def _send(self, result):
        body = json.dumps(result.body, separators=(",", ":")).encode("utf-8")
        self.send_response(result.status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)
        if result.command:
            print(f"[fixture] accepted command={result.command}", flush=True)

    def do_POST(self):
        transfer_encoding = self.headers.get("Transfer-Encoding", "")
        length_text = self.headers.get("Content-Length", "")
        if transfer_encoding or not length_text.isascii() or not length_text.isdecimal():
            self._send(_response(400, message="invalid_framing"))
            return
        length = int(length_text)
        if length > MAX_BODY:
            self._send(_response(413, message="too_large"))
            return
        body = self.rfile.read(length)
        result = validate_request(
            self.path,
            self.headers,
            body,
            self.server.expected_token,
            int(time.time() * 1000),
            self.server.seen,
        )
        self._send(result)

    def do_GET(self):
        self._send(_response(405, message="method_not_allowed"))

    def do_PUT(self):
        self._send(_response(405, message="method_not_allowed"))

    def log_message(self, fmt, *args):
        # Never print headers or request bodies; the Authorization header is secret.
        print(f"[fixture] {self.client_address[0]} {fmt % args}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1", help="bind address (loopback by default)")
    parser.add_argument("--port", type=int, default=8443)
    parser.add_argument("--cert", required=True, help="test server certificate PEM")
    parser.add_argument("--key", required=True, help="test server private key PEM")
    parser.add_argument("--token-env", default="MEDIA_TEST_TOKEN",
                        help="environment variable containing a disposable 64-hex token")
    args = parser.parse_args()
    token = os.environ.get(args.token_env, "")
    if not TOKEN_RE.fullmatch(token):
        parser.error(f"set {args.token_env} to a disposable 64-character lowercase hex token")

    server = HTTPServer((args.host, args.port), Handler)
    server.expected_token = token
    server.seen = {}
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(args.cert, args.key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print(f"[fixture] non-actioning HTTPS server listening on {args.host}:{args.port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
