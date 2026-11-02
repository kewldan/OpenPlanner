#!/usr/bin/env python3
"""MCP настоящего приложения: мост `<binary> mcp` запускает OpenPlanner (или
подключается к открытому), агент рисует квартиру, читает её и выгружает PDF и
IFC. Использование: scripts/desktop-mcp.py <путь к openplanner-desktop> [папка]."""

import json
import os
import subprocess
import sys
import tempfile
import time

binary = sys.argv[1]
out_dir = sys.argv[2] if len(sys.argv) > 2 else tempfile.mkdtemp()
bridge = subprocess.Popen(
    [binary, "mcp"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
    bufsize=1,
)
next_id = 0


def send(method, params=None, notify=False):
    global next_id
    message = {"jsonrpc": "2.0", "method": method}
    if params is not None:
        message["params"] = params
    if not notify:
        next_id += 1
        message["id"] = next_id
    bridge.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
    bridge.stdin.flush()
    if notify:
        return None
    line = bridge.stdout.readline()
    if not line:
        sys.exit("мост закрылся без ответа")
    reply = json.loads(line)
    assert reply.get("id") == next_id, reply
    return reply


def tool(name, **arguments):
    reply = send("tools/call", {"name": name, "arguments": arguments})
    result = reply["result"]
    text = result["content"][0]["text"]
    if result["isError"]:
        sys.exit(f"{name}: {text}")
    print(f"{name}: {text[:200]}")
    return json.loads(text)


started = time.time()
init = send(
    "initialize",
    {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "selftest", "version": "1"}},
)
print("сервер:", init["result"]["serverInfo"], f"({time.time() - started:.1f} с)")
send("notifications/initialized", notify=True)
names = [t["name"] for t in send("tools/list")["result"]["tools"]]
print("инструментов:", len(names))

tool("add_walls", points=[[0, 0], [5000, 0], [5000, 4000], [0, 4000]], closed=True, thickness=200)
tool("add_walls", points=[[2500, 0], [2500, 4000]])
tool("find_rooms")
tool("add_opening", kind="door", at=[2500, 2000])
tool("add_opening", kind="window", at=[1200, 0])
project = tool("get_project")
assert len(project["rooms"]) == 2, project
pdf = os.path.join(out_dir, "mcp-selftest.pdf")
ifc = os.path.join(out_dir, "mcp-selftest.ifc")
tool("export", format="pdf", path=pdf)
tool("export", format="ifc", path=ifc)
saved = tool("save_project", path=os.path.join(out_dir, "mcp-selftest.oplan"))

for path in [pdf, ifc, saved["path"]]:
    assert os.path.getsize(path) > 0, path
assert open(pdf, "rb").read(4) == b"%PDF"
assert b"IFCWALL(" in open(ifc, "rb").read()
print("OK", out_dir)
bridge.stdin.close()
bridge.wait(timeout=10)
