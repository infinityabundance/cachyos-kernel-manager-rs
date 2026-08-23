#!/usr/bin/env python3
"""i18n-drive.py — Phase 12 hostile-review rendered-i18n driver.

Projects the MAIN window's translation-sensitive accessible surface from
the STARTUP AT-SPI tree — the RENDERED production projection (audit P2:
the i18n courts must witness what the GUI actually renders, not just
catalog lookup). Side-agnostic: the same driver witnesses the frozen Qt
oracle and the release Slint candidate; both run under the SAME generated
locale (de_DE.UTF-8 — the translated surface; zh_CN.UTF-8 — gap-009's
rendered projection: both sides show English because Qt's QLocale reports
zh_CN while the frozen qrc alias is zh-CN).

The projection is deliberately the translation-sensitive chrome only:

- window_title: the main window's accessible name,
- description: the longest text node (the description is the only
  multi-sentence node in either GUI's main window),
- headers: the four tree column headers in visual order (Qt: the
  TABLE_COLUMN_HEADER roles inside the main window; Slint: the header
  strip's button-role nodes — the PUSH_BUTTONs before the first kernel
  row),
- buttons: the main window's action buttons in visual order (the
  PUSH_BUTTONs after the last kernel row).

The kernel rows themselves are deliberately NOT projected (the Qt bridge
exposes them as tree items, the Slint bridge as cells — structurally
incomparable raw trees); the ROW texts were already courted byte-for-byte
by ui/gui-drive.

Usage:
  i18n-drive.py <out-dir> <locale-key>            # oracle mode (wait for the
                                                  # app the wrapper launched)
  i18n-drive.py --candidate <binary> <out-dir> <locale-key>   # launch it
The locale env (LANG/LC_ALL) is set by the wrapper (oracle) or here
(candidate mode); KM_LOCALE selects the output filename.
"""

import json
import os
import re
import subprocess
import sys
import time
from collections import deque

import pyatspi

TIMEOUT = 150.0
POLL = 0.5

ROLE_NAMES = {
    0: "INVALID", 1: "ACCELERATOR_LABEL", 2: "ALERT", 3: "ANIMATION",
    4: "ARROW", 5: "CALENDAR", 6: "CANVAS", 7: "CHECK_BOX",
    8: "CHECK_MENU_ITEM", 9: "COLOR_CHOOSER", 10: "COLUMN_HEADER",
    11: "COMBO_BOX", 12: "DATE_EDITOR", 13: "DESKTOP_ICON",
    14: "DESKTOP_FRAME", 15: "DIAL", 16: "DIALOG", 17: "DIRECTORY_PANE",
    18: "DRAWING_AREA", 19: "FILE_CHOOSER", 20: "FILLER",
    21: "FOCUS_TRAVERSABLE", 22: "FONT_CHOOSER", 23: "FRAME",
    24: "GLASS_PANE", 25: "HTML_CONTAINER", 26: "ICON", 27: "IMAGE",
    28: "INTERNAL_FRAME", 29: "LABEL", 30: "LAYERED_PANE", 31: "LIST",
    32: "LIST_ITEM", 33: "MENU", 34: "MENU_BAR", 35: "MENU_ITEM",
    36: "OPTION_PANE", 37: "PAGE_TAB", 38: "PAGE_TAB_LIST", 39: "PANEL",
    40: "PASSWORD_TEXT", 41: "POPUP_MENU", 42: "PROGRESS_BAR",
    43: "PUSH_BUTTON", 44: "RADIO_BUTTON", 45: "RADIO_MENU_ITEM",
    46: "ROOT_PANE", 47: "ROW_HEADER", 48: "SCROLL_BAR",
    49: "SCROLL_PANE", 50: "SEPARATOR", 51: "SLIDER", 52: "SPIN_BUTTON",
    53: "SPLIT_PANE", 54: "STATUS_BAR", 55: "TABLE", 56: "TABLE_CELL",
    57: "TABLE_COLUMN_HEADER", 58: "TABLE_ROW_HEADER",
    59: "TEAROFF_MENU_ITEM", 60: "TERMINAL", 61: "TEXT",
    62: "TOGGLE_BUTTON", 63: "TOOL_BAR", 64: "TOOL_TIP", 65: "TREE",
    66: "TREE_TABLE", 67: "UNKNOWN", 68: "VIEWPORT", 69: "WINDOW",
    70: "EXTENDED", 71: "HEADER", 72: "FOOTER", 73: "PARAGRAPH",
    74: "RULER", 75: "APPLICATION", 76: "AUTOCOMPLETE", 77: "EDITBAR",
    78: "EMBEDDED", 79: "ENTRY", 80: "CHART", 81: "CAPTION",
    82: "DOCUMENT_FRAME", 83: "HEADING", 84: "PAGE", 85: "SECTION",
    86: "REDUNDANT_OBJECT", 87: "FORM", 88: "LINK",
    89: "INPUT_METHOD_WINDOW", 90: "TABLE_ROW", 91: "TREE_ITEM",
    92: "DOCUMENT_SPREADSHEET", 93: "DOCUMENT_PRESENTATION",
    94: "DOCUMENT_TEXT", 95: "DOCUMENT_WEB", 96: "DOCUMENT_EMAIL",
    97: "COMMENT", 98: "LIST_BOX", 99: "GROUPING", 100: "IMAGE_MAP",
    101: "NOTIFICATION", 102: "INFO_BAR", 103: "LEVEL_BAR",
    104: "TITLE_BAR", 105: "BLOCK_QUOTE", 106: "AUDIO", 107: "VIDEO",
    108: "DEFINITION", 109: "ARTICLE", 110: "LANDMARK", 111: "LOG",
    112: "MARQUEE", 113: "MATH", 114: "RATING", 115: "TIMER",
}

# the text-bearing roles the projection collects (the kernel rows are
# tree items on the Qt side and table cells on the Slint side — NEVER
# projected; only the chrome is)
KEPT_ROLES = {
    "LABEL", "TEXT", "PUSH_BUTTON", "BUTTON", "COLUMN_HEADER",
    "TABLE_COLUMN_HEADER",
}
BUTTON_ROLES = {"PUSH_BUTTON", "BUTTON"}
HEADER_ROLES = {"COLUMN_HEADER", "TABLE_COLUMN_HEADER"}
WINDOW_ROLES = {"FRAME", "WINDOW", "ROOT_PANE", "APPLICATION"}

VERSION_RE = re.compile(r"^\d+\.\d")
# a kernel row identity is "<repo>/<name>" — the slash must follow a
# repo-like token at the START (the translated description legitimately
# contains mid-text slashes, e.g. German "installieren/deinstallieren")
REPO_RE = re.compile(r"^[a-z][a-z0-9-]*/")
WAIT_RE = re.compile(r"Please wait", re.IGNORECASE)


def role_name(role):
    if role is None:
        return "?"
    try:
        return ROLE_NAMES[int(role)]
    except (TypeError, ValueError):
        return str(role).split(".")[-1]


def node_text(obj):
    try:
        return obj.queryText().getText(0, -1)
    except (NotImplementedError, AttributeError):
        try:
            return obj.name or ""
        except Exception:
            return ""


def find_app():
    desktop = pyatspi.Registry.getDesktop(0)
    for i in range(desktop.childCount):
        try:
            app = desktop[i]
        except Exception:
            continue
        try:
            app_name = (app.name or "").lower()
        except Exception:
            app_name = ""
        if (
            "cachyos" in app_name
            or "kernel" in app_name
            or app_name in ("cachyos-km", "cachyos-kernel-manager")
        ):
            return app
    return None


def wait_app():
    app = None
    deadline = time.time() + TIMEOUT
    while app is None and time.time() < deadline:
        app = find_app()
        if app is None:
            time.sleep(POLL)
    return app


def is_rowish(text):
    # a kernel row's package identity ("repo/name"), a package name, or a
    # version text — the chrome never contains these (the translated
    # description can contain mid-text slashes — German
    # "installieren/deinstallieren" — so the repo/ anchor must be at the
    # START)
    return bool(REPO_RE.match(text)) or "linux" in text or bool(VERSION_RE.match(text))


def collect(root):
    """BFS the tree keeping (role, text) of the text-bearing chrome roles."""
    kept = []
    queue = deque([root])
    while queue:
        node = queue.popleft()
        try:
            r = role_name(node.getRole())
        except Exception:
            continue
        if r in KEPT_ROLES:
            txt = node_text(node).strip()
            if txt:
                kept.append((node, r, txt))
        try:
            queue.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return kept


def window_root_of(desc_node):
    """The window-ish ancestor of the description node (FRAME/WINDOW/ROOT_PANE
    — Qt's main window frame; the Slint top-level window)."""
    cur = desc_node
    while True:
        parent = cur.parent
        if parent is None:
            return cur
        try:
            pr = role_name(parent.getRole())
        except Exception:
            return cur
        if pr in WINDOW_ROLES:
            return parent
        cur = parent


def wait_ready(app):
    """Wait until the app finishes the startup discovery — the in-window
    progress overlay ("Please wait...\nInitializing kernels..") disappears
    and the kernel rows render. The projection must happen AFTER this: the
    row nodes are the header/button classification boundary on the Slint
    side."""
    deadline = time.time() + 120.0
    while time.time() < deadline:
        done = True
        queue = deque([app])
        while queue:
            node = queue.popleft()
            try:
                if WAIT_RE.search(node_text(node)):
                    done = False
                    break
            except Exception:
                pass
            try:
                queue.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        if done:
            return
        time.sleep(POLL)


def project(app):
    kept = collect(app)
    if not kept:
        return None
    desc_node, _, _ = max(kept, key=lambda k: len(k[2]))
    # the main window root = the window-ish ancestor of the description
    # (Qt: the FRAME named "CachyOS Kernel Manager"; Slint: the top-level
    # window — the ONLY window at startup, so the app's other windows are
    # never inside this subtree)
    root = window_root_of(desc_node)
    kept = collect(root)
    desc_node, _, desc_text = max(kept, key=lambda k: len(k[2]))
    if os.environ.get("KM_I18N_DEBUG"):
        for i, (_, r, t) in enumerate(kept):
            print(f"i18n-debug[{i}] role={r} text={t[:80]!r}", flush=True)
    buttons = [k for k in kept if k[1] in BUTTON_ROLES]
    col_headers = [k for k in kept if k[1] in HEADER_ROLES]
    rowish = [k for k in kept if is_rowish(k[2])]
    if col_headers:
        headers = col_headers
        last_header = max(kept.index(h) for h in col_headers)
        buttons = [b for b in buttons if kept.index(b) > last_header]
    else:
        # Slint: the header strip's button-role nodes sit BEFORE the first
        # kernel row; the action buttons AFTER the last kernel row
        first_row = kept.index(rowish[0]) if rowish else None
        last_row = kept.index(rowish[-1]) if rowish else None
        if first_row is None:
            headers = []
            buttons = []
        else:
            headers = [b for b in buttons if kept.index(b) < first_row]
            buttons = [b for b in buttons if kept.index(b) > last_row]
    return {
        "window_title": (root.name or "").strip(),
        "description": desc_text,
        "headers": [t for (_, _, t) in headers],
        "buttons": [t for (_, _, t) in buttons],
    }


def write(out_dir, locale_key, payload):
    with open(os.path.join(out_dir, f"i18n-{locale_key}.json"), "w") as f:
        json.dump(payload, f, indent=1)


def main():
    args = sys.argv[1:]
    locale_key = os.environ.get("KM_LOCALE", "de_DE")
    if args and args[0] == "--candidate":
        binary = args[1]
        out_dir = args[2] if len(args) > 2 else "/mnt/host/out"
        proc = subprocess.Popen(
            [binary],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=dict(os.environ),
        )
        try:
            app = wait_app()
            if app is None:
                write(out_dir, locale_key, {"error": "no app"})
                return 1
            wait_ready(app)
            write(out_dir, locale_key, project(app))
        finally:
            try:
                proc.terminate()
                proc.wait(timeout=5)
            except Exception:
                try:
                    proc.kill()
                except Exception:
                    pass
        return 0
    out_dir = args[0] if args else "/mnt/host/out"
    app = wait_app()
    if app is None:
        write(out_dir, locale_key, {"error": "no app"})
        return 1
    wait_ready(app)
    write(out_dir, locale_key, project(app))
    return 0


if __name__ == "__main__":
    sys.exit(main())
