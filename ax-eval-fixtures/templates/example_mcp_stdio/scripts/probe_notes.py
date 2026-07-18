#!/usr/bin/env python3
import argparse
import json
import os
import sys


def load_notes(path):
    if not os.path.exists(path):
        return []
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--health", action="store_true")
    parser.add_argument("--require-note", action="append", default=[])
    args = parser.parse_args()

    store = os.environ.get("NOTE_STORE", "notes.json")

    if args.health:
        if not os.path.exists(store):
            with open(store, "w", encoding="utf-8") as handle:
                json.dump([], handle)
        print(json.dumps({"passed": True, "message": "note store is available"}))
        return 0

    notes = load_notes(store)
    texts = {str(note.get("text", "")) for note in notes}
    missing = [text for text in args.require_note if text not in texts]
    if missing:
        print(
            json.dumps(
                {
                    "passed": False,
                    "message": "missing required notes: " + ", ".join(missing),
                    "notes": notes,
                }
            )
        )
        return 1

    print(
        json.dumps(
            {
                "passed": True,
                "message": f"found {len(args.require_note)} required note(s)",
                "notes": notes,
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
