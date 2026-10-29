"""#1569: `RemoteTrigger` is called over `claude mcp serve`'s stdio, and its answers read.

The server is faked by a small Python process speaking the same JSON-RPC, so
the protocol (initialize, then one `tools/call`) runs for real and offline.
"""

from __future__ import annotations

import json
import sys
import unittest

from dispatch import remote

# A stand-in for `claude mcp serve`: answers `initialize`, then the tool call
# the way the mode in argv says.
FAKE = r'''
import json, sys
mode = sys.argv[1]
print("a log line, not the protocol", flush=True)
for line in sys.stdin:
    message = json.loads(line)
    ident = message.get("id")
    if message.get("method") == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": ident, "result": {}}), flush=True)
    elif message.get("method") == "tools/call":
        if mode == "silent":
            sys.exit(0)
        arguments = message["params"]["arguments"]
        if mode == "refuse":
            result = {"content": [{"type": "text", "text": "create requires body"}], "isError": True}
        elif mode == "garbage":
            result = {"content": [{"type": "text", "text": "not json"}]}
        else:
            body = json.dumps({"echo": arguments, "tool": message["params"]["name"]})
            result = {"content": [{"type": "text", "text": json.dumps({"status": 201, "json": body})}]}
        print(json.dumps({"jsonrpc": "2.0", "method": "notifications/progress"}), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "id": ident, "result": result}), flush=True)
'''


def serve(mode: str) -> tuple[str, ...]:
    return (sys.executable, "-c", FAKE, mode)


class Call(unittest.TestCase):
    def test_answers_the_status_and_the_parsed_body(self):
        status, body = remote.call({"action": "list"}, command=serve("ok"))
        self.assertEqual(status, 201)
        self.assertEqual(body, {"echo": {"action": "list"}, "tool": "RemoteTrigger"})

    def test_a_refused_call_raises_with_the_reason(self):
        with self.assertRaisesRegex(remote.RemoteError, "create requires body"):
            remote.call({"action": "create"}, command=serve("refuse"))

    def test_a_server_that_ends_without_answering_raises(self):
        with self.assertRaisesRegex(remote.RemoteError, "without answering"):
            remote.call({"action": "list"}, command=serve("silent"))

    def test_an_answer_that_is_not_the_tools_shape_raises(self):
        with self.assertRaisesRegex(remote.RemoteError, "unexpected"):
            remote.call({"action": "list"}, command=serve("garbage"))

    def test_a_missing_claude_raises(self):
        with self.assertRaisesRegex(remote.RemoteError, "not installed"):
            remote.call({"action": "list"}, command=("rusty-no-such-binary-1569",))

    def test_a_hung_server_is_killed_at_the_timeout(self):
        hang = (sys.executable, "-c", "import time; time.sleep(60)")
        with self.assertRaises(remote.RemoteError):
            remote.call({"action": "list"}, command=hang, timeout=0.5)

    def test_the_request_is_json_rpc(self):
        request = json.loads(remote._request(2, "tools/call", {"name": "RemoteTrigger"}))
        self.assertEqual(request, {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                                   "params": {"name": "RemoteTrigger"}})


if __name__ == "__main__":
    unittest.main()
