#!/usr/bin/env python3
"""oracle-configure.py — drive the REAL oracle Configure flow via AT-SPI.

Used by the git-cache/lifecycle court (Phase 6). Connects to the session's
at-spi registry, waits for the real `cachyos-kernel-manager` GUI, then:

1. dumps the FULL accessibility tree (same schema as oracle-drive.py) —
   the discovery rows of the SAME run that executes the Configure flow;
2. clicks the Configure button (PUSH_BUTTON whose name contains
   "configure") with a synthesized XTEST pointer event at its a11y-reported
   extents — the Qt bridge exposes no working action, same as the Execute
   button (verified in Phase 5);
3. waits for the prepare_build_environment git refresh execs
   (`git checkout --force master` / `git clean -fd` / `git pull`) to appear
   in the strace trace (evidence the refresh chain really started) and
   writes a marker file with the number of git execs seen.

Raw evidence discipline: only the tree dump + the marker are written; the
exec-chain evidence comes from the strace trace, extracted by
extract-git-cache.py.
"""

import ctypes
import ctypes.util
import json
import re
import sys
import time

import pyatspi

TIMEOUT = 90.0
POLL = 0.5

# AT-SPI role id -> canonical role name (same table as oracle-observe.py /
# oracle-drive.py; duplicated here so oracle-configure.py is standalone).
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
        return str(role).split(".")[-1]


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


def wait_for_rows(root):
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
                try:
                    txt = (node.queryText().getText(0, -1) + " " + (node.name or "")).lower()
                except Exception:
                    txt = (node.name or "").lower()
                if "kernel" in txt or "pacman" in txt:
                    return True
            try:
                stack.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        time.sleep(POLL)
    return False


def dump_node(obj, depth=0, max_depth=24):
    if depth > max_depth:
        return {"_truncated": True}
    role = obj.getRole()

    def text_of(o):
        try:
            return o.queryText().getText(0, -1)
        except (NotImplementedError, AttributeError):
            try:
                return o.name or ""
            except Exception:
                return ""

    def states_of(o):
        def name_of(s):
            return getattr(s, "name", str(s)).lower()

        try:
            return sorted(name_of(s) for s in o.getState().getStates())
        except Exception:
            return []

    node = {
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


def click_at(rect, display=":99"):
    """Synthesize a left click via XTEST at a11y-reported coordinates (the
    same activation path oracle-drive.py uses for the Execute button)."""
    try:
        x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
        xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
        x11.XOpenDisplay.restype = ctypes.c_void_p
        dpy = x11.XOpenDisplay(display.encode())
        if not dpy:
            return False
        cx = int(rect.x) + int(rect.width) // 2
        cy = int(rect.y) + int(rect.height) // 2
        xtst.XTestFakeMotionEvent(ctypes.c_void_p(dpy), -1, ctypes.c_int(cx), ctypes.c_int(cy), 0)
        xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 1, 0)
        xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 0, 0)
        x11.XFlush(ctypes.c_void_p(dpy))
        return True
    except Exception as e:
        print(f"WARN: XTEST click failed: {e}", file=sys.stderr)
        return False


def click_configure(app):
    """Find the Configure push button and click it (action if exposed,
    else an a11y-coordinate XTEST click)."""
    stack = [app]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
            nm = (node.name or "").lower()
        except Exception:
            r, nm = None, ""
        if r in ("PUSH_BUTTON", "43") and "configure" in nm:
            try:
                actions = node.getAction()
                if actions is not None and actions.nActions > 0:
                    for i in range(actions.nActions):
                        try:
                            actions.doAction(i)
                            return True
                        except Exception:
                            pass
            except Exception:
                pass
            try:
                comp = node.get_component()
                if comp is not None:
                    return click_at(comp.get_extents(0))
            except Exception as e:
                print(f"WARN: configure button click failed: {e}", file=sys.stderr)
                return False
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return False


def count_git_execs(trace_path):
    """Count the prepare_git_repo execs (clone/checkout/clean/pull) seen so
    far — the witness that the refresh chain really started."""
    try:
        with open(trace_path, "r", errors="replace") as f:
            return sum(
                1
                for line in f
                if 'execve("' in line
                and re.search(r'\["[^"]*git", "(clone|checkout|clean|pull)"', line)
            )
    except OSError:
        return 0


def main():
    out_state = sys.argv[1] if len(sys.argv) > 1 else "/tmp/oracle-state.json"
    trace_path = "/tmp/oracle-trace.log"
    marker = "/tmp/oracle-drive.marker"

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

    ok = wait_for_rows(app)

    # dump the tree of the SAME run (discovery rows), before any click
    tree = {"schema": "cachyos-km-oracle-a11y-v1", "observable": "full-at-spi-tree",
            "app_name": app.name or "", "rows_ready": ok, "children": [dump_node(app)]}
    with open(out_state, "w", encoding="utf-8") as f:
        json.dump(tree, f, indent=1, ensure_ascii=False)
    print(f"dumped a11y tree (rows_ready={ok})")

    if not ok:
        print("WARN: no kernel rows visible — still attempting the Configure click", file=sys.stderr)

    clicked = click_configure(app)
    print(f"configure click={'ok' if clicked else 'FAILED'}")

    # wait for the refresh execs (checkout/clean/pull) to appear
    seen = 0
    deadline = time.time() + TIMEOUT
    while time.time() < deadline:
        seen = count_git_execs(trace_path)
        if seen >= 3:
            break
        time.sleep(POLL)
    with open(marker, "w") as f:
        f.write(str(seen))
    print(f"git refresh execs seen: {seen}")

    if not clicked:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
