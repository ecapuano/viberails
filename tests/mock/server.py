#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["aiohttp"]
# ///

import argparse
import json
import logging
import random

from aiohttp import web

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s",
)

REASONS = [
    "Policy compliance verified",
    "Request approved by admin",
    "Within allowed parameters",
    "User has required permissions",
    "Action permitted by ruleset",
]

ERRORS = [
    "Rate limit exceeded",
    "Invalid authentication token",
    "Resource not found",
    "Permission denied",
    "Service temporarily unavailable",
]

RULES = [
    "allow-all",
    "deny-external",
    "require-auth",
    "rate-limit-100",
    "admin-only",
]


def create_app(allow: bool):
    async def handle_request(request):
        logging.info(
            "%s %s from %s",
            request.method,
            request.path,
            request.remote,
        )
        try:
            body = await request.json()
            logging.info("Request body:\n%s", json.dumps(body, indent=2))
        except Exception:
            pass
        if allow:
            return web.json_response({
                "success": True,
                "reason": random.choice(REASONS),
                "error": None,
                "rejected": False,
                "rule": random.choice(RULES),
            })
        else:
            return web.json_response({
                "success": False,
                "reason": None,
                "error": random.choice(ERRORS),
                "rejected": True,
                "rule": random.choice(RULES),
            })

    app = web.Application()
    app.router.add_route("*", "/{path:.*}", handle_request)
    return app


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Mock cloud response server")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--allow", action="store_true",
                       help="Return success responses")
    group.add_argument("--deny", action="store_true",
                       help="Return failure responses")
    parser.add_argument("--port", type=int, default=8080,
                        help="Port to listen on")
    args = parser.parse_args()

    app = create_app(allow=args.allow)
    web.run_app(app, host="127.0.0.1", port=args.port)
