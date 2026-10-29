"""The claude.ai routines API, through `claude mcp serve` (#1569).

A cloud coder is a one-off routine. Its API lives behind Claude Code's
`RemoteTrigger` tool, which adds the claude.ai login's OAuth token in-process,
so the token never reaches a shell, a script or the repository. Three ways in
were tried on 2026-10-29 (Claude Code 2.1.312), and this is the one that works
from a script:

- `claude --cloud "<task>"` starts a cloud session only from an interactive
  terminal ("To start a new cloud session, run from a TTY"), and opens it in that
  terminal; with `-p` it only messages an existing session. An orchestrator's
  shell has no TTY. `--environment` takes only self-hosted `ccpool_` ids.
- A headless `claude -p --tools=RemoteTrigger` session works, but a model copies
  the body, which is a chance to change it, and it costs a model call.
- `claude mcp serve` lists `RemoteTrigger` among its tools. A `tools/call` over
  its stdio runs the tool with the login's token and no model in between, so
  the call is exactly the body this package built. That is what [`call`] does.

The tool answers `{"status": <HTTP status>, "json": "<body>"}` as text, or an
`isError` result naming what was wrong with the arguments.
"""

from __future__ import annotations

import json
import subprocess
import threading

#: Claude Code's own tools as an MCP server, over stdio.
SERVE = ("claude", "mcp", "serve")

#: The longest a call may take: start-up plus one HTTP round trip.
TIMEOUT_SECONDS = 90

PROTOCOL = "2025-06-18"


class RemoteError(RuntimeError):
    """The routines API could not be reached, or refused the call."""


def _request(ident: int, method: str, params: dict) -> str:
    return json.dumps({"jsonrpc": "2.0", "id": ident, "method": method, "params": params}) + "\n"


def _answer(stdout, ident: int) -> dict:
    """The response to request `ident`, skipping notifications and other ids."""
    for line in stdout:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue  # a log line on stdout is not the protocol
        if message.get("id") == ident:
            if "error" in message:
                raise RemoteError(f"claude mcp serve: {message['error'].get('message', message['error'])}")
            return message.get("result") or {}
    raise RemoteError("claude mcp serve ended without answering")


def call(arguments: dict, command: tuple[str, ...] = SERVE, timeout: float = TIMEOUT_SECONDS) -> tuple[int, object]:
    """Run `RemoteTrigger` with `arguments`; the HTTP status and the parsed body.

    Raises [`RemoteError`] when `claude` is missing, times out, or the tool
    refuses the arguments. A non-2xx status is returned, not raised: the caller
    says what it meant.
    """
    try:
        process = subprocess.Popen(list(command), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, text=True)
    except FileNotFoundError as missing:
        raise RemoteError(f"{command[0]} is not installed: {missing}") from missing
    timer = threading.Timer(timeout, process.kill)
    timer.start()
    try:
        process.stdin.write(_request(1, "initialize", {
            "protocolVersion": PROTOCOL, "capabilities": {},
            "clientInfo": {"name": "rusty-dispatch", "version": "1"}}))
        process.stdin.flush()
        _answer(process.stdout, 1)
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        process.stdin.write(_request(2, "tools/call", {"name": "RemoteTrigger", "arguments": arguments}))
        process.stdin.flush()
        result = _answer(process.stdout, 2)
    except (BrokenPipeError, OSError) as broken:
        raise RemoteError(f"claude mcp serve: {broken}") from broken
    finally:
        timer.cancel()
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        process.kill()
        process.wait()
        process.stdout.close()
    text = "".join(part.get("text", "") for part in result.get("content") or [])
    if result.get("isError"):
        raise RemoteError(f"RemoteTrigger refused the call: {text}")
    try:
        answer = json.loads(text)
        body = answer.get("json")
        return int(answer["status"]), json.loads(body) if body else None
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as odd:
        raise RemoteError(f"RemoteTrigger answered something unexpected: {text[:300]}") from odd
