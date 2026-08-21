#!/usr/bin/env python3
"""oracle-observe.py — dump the oracle's user-visible state via AT-SPI.

Runs inside the court VM. Connects to the session's at-spi registry, waits
for the real `cachyos-kernel-manager` GUI, then serializes its ENTIRE
accessibility tree (role, name, text, states, children) to JSON.

This is the primary observation channel for the baseline courts: the tree's
PkgName / Version / Category columns, checkbox states, hidden columns, and
any dialogs ("No kernels found!"...) are all observable without touching the
oracle binary. Screenshots are never used as evidence (directive §0, §37).

The dump is raw evidence: PIDs, dbus names and window coordinates are
present but must be normalized by the court comparators, never edited here.
"""

import json
import sys
import time

import pyatspi

TARGET = "cachyos-kernel-manager"
TIMEOUT = 90.0  # seconds to wait for the app + populated tree
POLL = 0.5

# AT-SPI role id -> canonical role name (at-spi2-core 2.60 / the vendored
# pyatspi2 role.py). pyatspi2's Accessible.getRole() returns plain ints on
# this stack, so string comparisons must use the numeric ids. The ids are
# the canonical evidence; names are for humans and comparators.
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


def role_name(role):
    if role is None:
        return "?"
    try:
        return ROLE_NAMES[int(role)]
    except (TypeError, ValueError):
        # already a name or an enum object
        return str(role).split(".")[-1]


def states_of(obj):
    def name_of(s):
        # pyatspi2 returns StateType enums; normalize to lowercase strings
        return getattr(s, "name", str(s)).lower()

    return sorted(name_of(s) for s in obj.getState().getStates())


def text_of(obj):
    try:
        return obj.queryText().getText(0, -1)
    except (NotImplementedError, AttributeError):
        try:
            return obj.name or ""
        except Exception:
            return ""


def dump_node(obj, depth=0, max_depth=24):
    if depth > max_depth:
        return {"_truncated": True}
    role = obj.getRole()
    node = {
        # canonical raw evidence: the numeric AT-SPI role id (stable across
        # at-spi2-core versions and the existing a11y schema), plus the
        # human-readable name for comparators
        "role": str(int(role)) if role is not None else "?",
        "role_name": role_name(role),
        "name": (obj.name or "")[:200],
        "text": text_of(obj)[:500],
        "states": states_of(obj),
        "children": [],
    }
    try:
        n = obj.childCount
    except Exception:
        n = 0
    for i in range(min(n, 400)):
        try:
            child = obj[i]
        except Exception:
            continue
        node["children"].append(dump_node(child, depth + 1, max_depth))
    return node


def find_app():
    desktop = pyatspi.Registry.getDesktop(0)
    for i in range(desktop.childCount):
        try:
            app = desktop[i]
        except Exception:
            continue
        # match by a11y application name: Qt registers the QApplication
        # name ("CachyOS-KM", set in main.cpp:126), NOT the executable name
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


def wait_for_rows(root):
    """Wait until the kernel tree has at least one row (or a dialog shows)."""
    # Qt's a11y bridge exposes the kernel QTreeWidget as a TREE (role 65)
    # or TREE_TABLE (66). Rows are TABLE_CELLs (56) directly under it
    # (flat layout) or TREE_ITEM/TABLE_ROW children (nested layout).
    TREE_ROLES = {"65", "66", "TREE", "TREE_TABLE"}
    ROW_ROLES = {"56", "90", "91", "TABLE_CELL", "TABLE_ROW", "TREE_ITEM"}
    DIALOG_ROLES = {"2", "16", "69", "ALERT", "DIALOG", "WINDOW"}
    deadline = time.time() + TIMEOUT
    while time.time() < deadline:
        stack = [root]
        while stack:
            node = stack.pop()
            try:
                r = role_name(node.getRole())
            except Exception:
                continue
            if r in TREE_ROLES:
                try:
                    kids = [node[i] for i in range(node.childCount)]
                except Exception:
                    kids = []
                for k in kids:
                    try:
                        kr = role_name(k.getRole())
                    except Exception:
                        kr = None
                    if kr in ROW_ROLES:
                        return True
            if r in DIALOG_ROLES:
                txt = (text_of(node) + " " + (node.name or "")).lower()
                if "kernel" in txt or "pacman" in txt:
                    return True
            try:
                stack.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        time.sleep(POLL)
    return False


def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/oracle-state.json"
    started = time.time()
    app = None
    while time.time() - started < 30:
        app = find_app()
        if app is not None:
            break
        time.sleep(POLL)
    if app is None:
        print("FATAL: oracle application not found on the at-spi registry", file=sys.stderr)
        return 1

    root = app
    ok = wait_for_rows(app)
    tree = dump_node(app)

    payload = {
        "schema": "cachyos-km-oracle-a11y-v1",
        "observable": "full-at-spi-tree",
        "app_name": app.name or "",
        "rows_populated": ok,
        "tree": tree,
    }
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=1, ensure_ascii=False)
    print(f"oracle state dumped to {out_path} (rows_populated={ok})")
    return 0 if ok else 2


if __name__ == "__main__":
    sys.exit(main())
