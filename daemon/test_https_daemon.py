#!/usr/bin/env python3
"""Send one fresh, certificate-verified localhost request to the daemon."""

import argparse
import http.client
import json
import ssl
import time
import uuid


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ca", required=True, help="exported trusted-ca.pem")
    parser.add_argument("--token", required=True, help="pairing token shown by the daemon")
    parser.add_argument("--host", default="localhost", help="localhost or 127.0.0.1")
    parser.add_argument("--port", type=int, default=8443)
    parser.add_argument("--command", choices=("play", "pause", "next", "previous", "mute", "unmute"), default="pause")
    parser.add_argument("--action-id", help="send to /v1/actions instead of /v1/media")
    args = parser.parse_args()

    issued = int(time.time() * 1000)
    request = {
        "id": uuid.uuid4().hex,
        "issued_at_ms": issued,
        "expires_at_ms": issued + 5000,
    }
    if args.action_id:
        request["action_id"] = args.action_id
        path = "/v1/actions"
    else:
        request["cmd"] = args.command
        path = "/v1/media"

    context = ssl.create_default_context(cafile=args.ca)
    connection = http.client.HTTPSConnection(args.host, args.port, context=context, timeout=3)
    body = json.dumps(request, separators=(",", ":")).encode("utf-8")
    try:
        connection.request(
            "POST",
            path,
            body=body,
            headers={
                "Authorization": f"Bearer {args.token}",
                "Content-Type": "application/json",
                "Content-Length": str(len(body)),
            },
        )
        response = connection.getresponse()
        payload = response.read(512)
        print(f"HTTP {response.status}: {payload.decode('utf-8', errors='replace')}")
        return 0 if response.status == 200 else 1
    finally:
        connection.close()


if __name__ == "__main__":
    raise SystemExit(main())
