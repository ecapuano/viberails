#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["aiohttp"]
# ///
"""
Mock server for testing the viberails client locally.

Accepts any HTTP method and path, logs the request, and returns {"status": "ok"}.
"""

import json

from aiohttp import web

HOST = "localhost"
PORT = 8000


async def catch_all_handler(request: web.Request) -> web.Response:
    """Handle any request to any path."""
    path = request.path
    method = request.method
    try:
        data = await request.json()
        print(f"[{method} {path}] Received: {json.dumps(data, indent=2)}")
    except json.JSONDecodeError:
        print(f"[{method} {path}] Received request (no JSON body)")

    return web.json_response({"allow": True, "reason": ""})


def create_app() -> web.Application:
    app = web.Application()
    app.router.add_route("*", "/{path:.*}", catch_all_handler)
    return app


if __name__ == "__main__":
    print(f"Starting mock server on http://{HOST}:{PORT}")
    print("Accepts any HTTP method and path")
    print()
    app = create_app()
    web.run_app(app, host=HOST, port=PORT)
