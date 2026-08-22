#!/usr/bin/env python3
"""extract-transaction.py — extract the exec-chain witness from a strace log.

Reads a strace `-f -e trace=execve,execveat -s 256` log (oracle-trace.log)
and produces `oracle-transaction.json` (schema `cachyos-km-oracle-transaction-v1`):

    probes   — exec chains of the probe commands, in execution order:
               findmnt, the chwd pipeline (sh + chwd + grep + awk), and the
               install-time `pacman -Qqs` module probes (sh + pacman).
               These are the oracle's *static-init* probes (kernel.cpp:41-52)
               plus the *install-time* probes (kernel.cpp:114-115).
    execs    — the transaction pacman execs (`pacman -S --needed ...`,
               `pacman -Rsn ...`), in execution order.
    terminal — the terminal-helper invocation argv.

The raw trace is preserved alongside (never edited): this file is the
explicit, versioned normalizer (directive §46).
"""

import json
import re
import sys


def parse_execve_line(line):
    """Return (argv_list, ok) for an execve line, or (None, False).
    Handles the strace `/* N vars */` environment-count suffix after the
    stack address and `<unfinished ...>` lines (argv is still complete)."""
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


PROBE_SH_MARKERS = (
    "findmnt -ln -o FSTYPE",
    "chwd --list-installed",
    "pacman -Qqs",
)
PROBE_INNER = {"findmnt", "chwd", "grep", "awk"}
PROBE_PACMAN_Q = ("-Qqs",)


def classify(argv):
    """(kind, include) — kind in probes/execs/terminal."""
    if not argv:
        return None, False
    a0 = argv[0]
    if "terminal-helper" in a0:
        return "terminal", True
    if a0 == "pacman" and len(argv) > 1:
        if argv[1] in ("-S", "-Rsn", "-Syu", "-U", "-R"):
            return "exec", True
        if argv[1] in PROBE_PACMAN_Q:
            return "probe", True
    if a0 == "sh" and len(argv) > 2 and argv[1] == "-c":
        # glibc ≥ 2.44 popen argv: sh -c -- <cmd>
        cmd_idx = 3 if len(argv) > 3 and argv[2] == "--" else 2
        if any(m in argv[cmd_idx] for m in PROBE_SH_MARKERS):
            return "probe", True
    if a0 in PROBE_INNER:
        return "probe", True
    return None, False


def main():
    trace_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/oracle-trace.log"
    out_path = sys.argv[2] if len(sys.argv) > 2 else "/tmp/oracle-transaction.json"

    probes = []
    execs = []
    terminal = None
    with open(trace_path, "r", errors="replace") as f:
        for line in f:
            argv, ok = parse_execve_line(line)
            if not ok:
                continue
            kind, include = classify(argv)
            if not include:
                continue
            if kind == "probe":
                probes.append({"argv": argv})
            elif kind == "exec":
                execs.append({"argv": argv})
            elif kind == "terminal" and terminal is None:
                terminal = {"argv": argv}

    payload = {
        "schema": "cachyos-km-oracle-transaction-v1",
        "probes": probes,
        "execs": execs,
        "terminal": terminal,
    }
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=1, ensure_ascii=False)
    print(
        f"extracted {len(probes)} probe chains, {len(execs)} exec chains, "
        f"terminal={'yes' if terminal else 'no'}"
    )
    return 0 if execs else 2


if __name__ == "__main__":
    sys.exit(main())
