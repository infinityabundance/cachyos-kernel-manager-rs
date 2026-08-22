#!/usr/bin/env python3
"""extract-git-cache.py — extract the git refresh exec chain from a strace log.

Reads a strace `-f -e trace=execve,execveat -s 256` log (oracle-trace.log)
produced by oracle-configure.sh and writes `oracle-transaction.json`
(schema `cachyos-km-oracle-transaction-v1`) containing ONLY the git execs,
in execution order:

    execs — `git checkout --force master`, `git clean -fd`, `git pull`
            (and `git clone ...` when the fixture forces the clone path)

The probes (findmnt/chwd/pacman -Qqs) are not part of this court's claim
(they are courted elsewhere); the candidate model likewise emits no probes.
The raw trace is preserved alongside (never edited): this file is the
explicit, versioned normalizer (directive §46).
"""

import json
import re
import sys
import os

# The prepare_git_repo commands (utils.cpp:177-193) — the ONLY git execs the
# court claims. Git's internal helpers (git-core/git fetch, pack-objects,
# merge, maintenance, rev-list, git-upload-pack, ...) are implementation
# internals, not part of the oracle's argv contract.
PREPARE_GIT_COMMANDS = {"clone", "checkout", "clean", "pull"}


def parse_execve_line(line):
    """Return (argv_list, ok) for an execve line. Same parser as
    extract-transaction.py (handles the `/* N vars */` env-count suffix and
    `<unfinished ...>` lines)."""
    m = re.match(
        r'^\s*\d+\s+execve(at)?\("((?:[^"\\]|\\.)*)",\s*\[(.*)\](?:,.*)?\)?\s*(?:=\s*-?\d+|\s*<unfinished[^>]*>)?',
        line,
    )
    if not m:
        return None, False
    body = m.group(3)
    argv = []
    i = 0
    n = len(body)
    while i < n:
        if body[i] != '"':
            i += 1
            continue
        i += 1
        buf = []
        while i < n:
            c = body[i]
            if c == "\\":
                nxt = body[i + 1] if i + 1 < n else ""
                buf.append({"n": "\n", "t": "\t", "r": "\r", '"': '"', "\\": "\\"}.get(nxt, nxt))
                i += 2
            elif c == '"':
                i += 1
                break
            else:
                buf.append(c)
                i += 1
        argv.append("".join(buf))
    return argv, True


def main():
    trace_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/oracle-trace.log"
    out_path = sys.argv[2] if len(sys.argv) > 2 else "/tmp/oracle-transaction.json"

    execs = []
    with open(trace_path, "r", errors="replace") as f:
        for line in f:
            argv, ok = parse_execve_line(line)
            if not ok or not argv:
                continue
            # normalize argv[0] to its basename: execvp resolves the program
            # path, so the oracle invokes it as "/usr/bin/git" while the
            # candidate model emits "git" — the basename IS the contract.
            prog = os.path.basename(argv[0])
            if prog == "git" and len(argv) > 1 and argv[1] in PREPARE_GIT_COMMANDS:
                norm = [prog] + argv[1:]
                execs.append({"argv": norm})

    # no terminal-helper runs in the Configure flow; the KEY is omitted
    # entirely because a bare `null` normalizes asymmetrically against the
    # candidate schema's nullable-array semantics.
    payload = {
        "schema": "cachyos-km-oracle-transaction-v1",
        "probes": [],
        "execs": execs,
    }
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=1, ensure_ascii=False)
    print(f"extracted {len(execs)} git exec chains")
    return 0 if execs else 2


if __name__ == "__main__":
    sys.exit(main())
