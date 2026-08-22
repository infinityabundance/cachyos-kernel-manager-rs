#!/usr/bin/env python3
"""update-status-tables.py — regenerate the phase-status tables in README.md
and docs/ARCHITECTURE.md from the SINGLE authority atlas/status.json.

Usage:
  tools/update-status-tables.py            regenerate in place
  tools/update-status-tables.py --check    regenerate to a temp copy and exit
                                           nonzero on any diff (CI gate)

The phase tables in the docs are DERIVED presentations; never hand-edit
them. The policy + status vocabulary live in status.json.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATUS = json.loads((ROOT / "atlas" / "status.json").read_text())

LABEL = {
    "pending": "pending",
    "in-progress": "in progress",
    "sealed": "**sealed**",
    "implemented": "**implemented**",
}


def phase_rows() -> list[str]:
    rows = []
    for p in STATUS["phases"]:
        rows.append(
            f"| {p['phase']} | {p['name']} | {LABEL[p['status']]} | {p['wording']} |"
        )
    return rows


def binary_note() -> str:
    return STATUS["binary_status"]


def render_readme_table() -> str:
    lines = [
        "| phase | scope | status |",
        "|---|---|---|",
    ]
    lines += phase_rows()
    return "\n".join(lines) + "\n"


def render_architecture_table() -> str:
    lines = [
        "| phase | scope | status |",
        "|---|---|---|",
    ]
    lines += phase_rows()
    return "\n".join(lines) + "\n"


def replace_section(text: str, start: str, end: str, new: str) -> str:
    i = text.index(start)
    try:
        j = text.index(end, i)
    except ValueError:
        j = len(text)  # the section runs to EOF
    return text[:i] + start + "\n\n" + new + "\n" + text[j:]


def regenerate() -> dict[Path, str]:
    readme = ROOT / "README.md"
    arch = ROOT / "docs" / "ARCHITECTURE.md"
    out = {}

    t = readme.read_text()
    # the README status section runs from the `## Status` header to the
    # next `## ` header
    new_readme = replace_section(t, "## Status", "\n## ", render_readme_table())
    # append the binary-status note right after the table (idempotent)
    note = f"> {binary_note()}"
    if note not in new_readme:
        new_readme = new_readme.replace(
            "\n\n## ", f"\n\n{note}\n\n## ", 1
        )
    out[readme] = new_readme

    t = arch.read_text()
    new_arch = replace_section(t, "## Phase status", "\n## ", render_architecture_table())
    out[arch] = new_arch
    return out


def main() -> int:
    check = "--check" in sys.argv
    generated = regenerate()
    if check:
        bad = []
        for path, content in generated.items():
            if path.read_text() != content:
                bad.append(str(path))
        if bad:
            print("status tables DRIFTED; run tools/update-status-tables.py")
            for b in bad:
                print(f"  {b}")
            return 1
        print("status tables up to date (atlas/status.json authority)")
        return 0
    for path, content in generated.items():
        path.write_text(content)
        print(f"updated {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
