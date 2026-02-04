#!/usr/bin/env python3
"""
Test framework for viberails binary commands.

Runs the viberails binary from dist/ with various commands to validate
functionality against a test environment.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path


def get_binary_name() -> str:
    """Get the binary name based on current platform and architecture."""
    import platform

    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "linux":
        if machine in ("x86_64", "amd64"):
            return "viberails-linux-x64"
        elif machine in ("aarch64", "arm64"):
            return "viberails-linux-arm64"
    elif system == "darwin":
        if machine in ("x86_64", "amd64"):
            return "viberails-macos-x64"
        elif machine in ("aarch64", "arm64"):
            return "viberails-macos-arm64"
    elif system == "windows":
        if machine in ("x86_64", "amd64"):
            return "viberails-windows-x64.exe"
        elif machine in ("aarch64", "arm64"):
            return "viberails-windows-arm64.exe"

    raise RuntimeError(f"Unsupported platform: {system}/{machine}")


def get_binary_path(dist_root: Path | None = None) -> Path:
    """Get path to the viberails binary in dist/."""
    if dist_root is None:
        project_root = Path(__file__).parent.parent
        dist_root = project_root / "dist"
    return dist_root / get_binary_name()


def exec(cmd_line: str,
         cwd: str | None = None,
         env: dict | None = None,
         check: bool = True,
         capture_stdout: bool = True,
         stdin_data: str | None = None) -> tuple[int, str, str]:
    """Execute a shell command and return (returncode, stdout, stderr)."""

    ret = 1
    out_str = ""
    out_err = ""

    env_copy = os.environ.copy()

    if env is not None:
        env_copy |= env

    if stdin_data is not None:
        stdin = subprocess.PIPE
    else:
        stdin = None

    if capture_stdout:
        p = subprocess.Popen(cmd_line,
                             shell=True,
                             text=True,
                             env=env_copy,
                             stdin=stdin,
                             stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE,
                             cwd=cwd)

    else:
        p = subprocess.Popen(cmd_line,
                             shell=True,
                             text=True,
                             stdin=stdin,
                             env=env_copy,
                             cwd=cwd)

    try:
        out_str, out_err = p.communicate(input=stdin_data)

        ret = p.returncode

        if check and 0 != ret:
            raise AssertionError(cmd_line, ret, out_str, out_err)
    finally:
        p.wait()

    return ret, out_str, out_err


def test_stdin(binary: Path, subdir: str) -> None:
    """Test a callback command with all JSON files in mock_data/<subdir>/."""
    mock_data_dir = Path(__file__).parent / "mock_data" / subdir

    if not mock_data_dir.exists():
        raise FileNotFoundError(
            f"Mock data directory not found: {mock_data_dir}")

    json_files = sorted(mock_data_dir.glob("*.json"))

    if not json_files:
        raise FileNotFoundError(f"No JSON files found in: {mock_data_dir}")

    # Calculate max filename length for alignment
    max_name_len = max(len(f.name) for f in json_files)

    print(f"{subdir}")

    for json_file in json_files:
        # Read and compact JSON to single line (claude-callback reads one line from stdin)
        with open(json_file) as f:
            data = json.load(f)
        stdin_data = json.dumps(data)

        start = time.perf_counter()
        returncode, stdout, stderr = exec(
            f"{binary} claude-callback",
            stdin_data=stdin_data,
            check=False
        )
        elapsed_ms = int((time.perf_counter() - start) * 1000)

        decision = json.loads(stdout).get(
            "decision", "unknown") if stdout.strip() else "error"
        print(
            f"    {json_file.name:<{max_name_len}}  exit={returncode}  decision={decision:<8}  time={elapsed_ms}ms")


def main() -> int:

    status = 1

    parser = argparse.ArgumentParser(
        description="Test runner for viberails binary")

    parser.add_argument("--org-id",
                        default=os.environ.get("OID"),
                        help="Organization ID (or set OID env var)")

    parser.add_argument("--secret-id",
                        default=os.environ.get("SECRET_ID"),
                        help="Secret ID (or set SECRET_ID env var)")

    parser.add_argument("-b", "--bin",
                        type=Path,
                        required=True,
                        help="Root directory containing dist binaries (default: <project>/dist)")

    args = parser.parse_args()

    try:

        binary = get_binary_path(args.bin)

        if not binary.exists():
            raise FileNotFoundError(f"Binary not found: {binary}")

        if not args.org_id or not args.secret_id:
            raise ValueError("--org-id and --secret-id required")

        # Join team
        team_url = f"https://0651b4f82df0a29c.hook.limacharlie.io/{args.org_id}/viberails/{args.secret_id}"
        exec(f"{binary} join-team {team_url}")

        test_stdin(binary, "claude_code")

        status = 0
    except KeyboardInterrupt:
        pass

    return status


if __name__ == "__main__":

    status = main()

    if 0 != status:
        sys.exit(status)
