#!/usr/bin/env python3
import json
import os
import sys


STORE = os.environ.get("NOTE_STORE", "notes.json")


def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("utf-8").strip()
        if line == "":
            break
        key, _, value = line.partition(":")
        headers[key.lower()] = value.strip()

    length = int(headers.get("content-length", "0"))
    if length <= 0:
        return None
    return json.loads(sys.stdin.buffer.read(length).decode("utf-8"))


def write_message(payload):
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("ascii"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()


def load_notes():
    if not os.path.exists(STORE):
        return []
    with open(STORE, "r", encoding="utf-8") as handle:
        return json.load(handle)


def save_notes(notes):
    with open(STORE, "w", encoding="utf-8") as handle:
        json.dump(notes, handle, indent=2)


def tool_result(data):
    return {
        "content": [{"type": "text", "text": json.dumps(data, sort_keys=True)}],
        "structuredContent": data,
    }


def handle_tool_call(params):
    name = params.get("name")
    arguments = params.get("arguments") or {}

    if name == "add_note":
        text = str(arguments.get("text", "")).strip()
        if not text:
            raise ValueError("text is required")
        notes = load_notes()
        note = {"id": len(notes) + 1, "text": text}
        notes.append(note)
        save_notes(notes)
        return tool_result({"note": note})

    if name == "list_notes":
        return tool_result({"notes": load_notes()})

    raise ValueError(f"unknown tool: {name}")


TOOLS = [
    {
        "name": "add_note",
        "description": "Persist a short note in the fixture note store.",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string", "description": "Note text"}},
            "required": ["text"],
        },
    },
    {
        "name": "list_notes",
        "description": "Return all notes currently saved in the fixture note store.",
        "inputSchema": {"type": "object", "properties": {}},
    },
]


while True:
    request = read_message()
    if request is None:
        break

    method = request.get("method")
    request_id = request.get("id")

    if request_id is None:
        continue

    try:
        if method == "initialize":
            result = {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "notes", "version": "0.1.0"},
                "instructions": "Use add_note to save notes and list_notes to verify them.",
            }
        elif method == "tools/list":
            result = {"tools": TOOLS}
        elif method == "tools/call":
            result = handle_tool_call(request.get("params") or {})
        else:
            raise ValueError(f"unsupported method: {method}")

        write_message({"jsonrpc": "2.0", "id": request_id, "result": result})
    except Exception as exc:
        write_message(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32000, "message": str(exc)},
            }
        )
