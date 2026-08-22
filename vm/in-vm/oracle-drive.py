#!/usr/bin/env python3
"""oracle-drive.py — drive a real oracle transaction via AT-SPI.

Used by transaction courts (Phase 5). Connects to the session's at-spi
registry, waits for the real `cachyos-kernel-manager` GUI, then:

1. dumps the FULL accessibility tree (same schema as oracle-observe.py) —
   the discovery rows of the SAME run that executes the transaction;
2. for each target row (argv: raw `<repo>/<kernel>`), toggles its checkbox
   from its current state (installed+immutable rows are checked by default,
   so toggling them starts a REMOVAL; uninstalled rows start unchecked, so
   toggling starts an INSTALL — exactly what the comparator.toml `select`
   list means);
3. clicks the Execute button;
4. waits for the first `pacman` execve to appear in the strace trace
   (evidence the transaction chain really started) and writes a marker file
   with the number of pacman execs seen.

The checkbox toggle prefers the AT-SPI Action interface ("toggle"/"click"
on the cell); if the Qt bridge exposes none, it falls back to focusing the
kernels tree and synthesizing a Space key (the oracle binds Space to
`check_uncheck_item`, km-window.cpp:223-226,243).

Raw evidence discipline: only the tree dump + the marker are written; the
exec-chain evidence comes from the strace trace, extracted by
extract-transaction.py.
"""

import ctypes
import ctypes.util
import json
import sys
import time

import pyatspi

TIMEOUT = 90.0
POLL = 0.5

# AT-SPI role id -> canonical role name (same table as oracle-observe.py;
# duplicated here so oracle-drive.py is standalone).
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


def cell_text(node):
    """Cell/column text: queryText with the `obj.name` fallback (the a11y
    bridge on this stack raises from queryText for some table cells; the
    name carries the same text — dump_node relies on the same fallback)."""
    try:
        return node.queryText().getText(0, -1)
    except Exception:
        try:
            return node.name or ""
        except Exception:
            return ""


def find_tree(app, target):
    """The kernels tree: the TREE that contains a cell with text == target.
    The app exposes several trees (the config window's patch trees come
    earlier in the a11y traversal), so the target row disambiguates."""
    def descendants_with_text(node, needle):
        stack = [node]
        while stack:
            n = stack.pop()
            if cell_text(n) == needle:
                return True
            try:
                stack.extend(n[i] for i in range(n.childCount))
            except Exception:
                pass
        return False

    stack = [app]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
        except Exception:
            continue
        if r in ("TREE", "TREE_TABLE", "65", "66"):
            if descendants_with_text(node, target):
                return node
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return None


def find_cell_by_text(tree, target):
    """The cell whose text == target (PkgName column), searching the whole
    tree (Qt's a11y bridge can nest cells under FILLER nodes)."""
    stack = [tree]
    while stack:
        node = stack.pop()
        if cell_text(node) == target:
            return node
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return None


def cell_checkbox(cell):
    """The checkbox cell of the row that `cell` (the PkgName cell) belongs
    to. Qt's a11y bridge exposes the Toggle ACTION on cells but it does not
    actually toggle QTreeWidgetItem checkboxes (verified: the Execute button
    stays disabled), so the checkbox must be clicked with a real pointer
    event at its a11y-reported extents. The cells are flat siblings under
    the tree: [4 headers, row1: checkbox,pkg,version,category, row2: ...],
    so the checkbox is the sibling directly before the pkg cell."""
    try:
        parent = cell.parent
        idx = None
        for i in range(parent.childCount):
            try:
                if parent[i] == cell:
                    idx = i
                    break
            except Exception:
                pass
        if idx is not None and idx > 0:
            return parent[idx - 1]
    except Exception:
        pass
    return cell


def toggle_row_click(cell):
    """Click the row's checkbox INDICATOR with XTEST (a11y-reported
    extents; the indicator sits at the left edge of the checkbox cell)."""
    box = cell_checkbox(cell)
    try:
        comp = box.get_component()
        if comp is None:
            return False
        return click_at(comp.get_extents(0), left_edge=True)
    except Exception as e:
        print(f"WARN: checkbox click failed: {e}", file=sys.stderr)
        return False


def toggle_via_action(obj):
    try:
        actions = obj.getAction()
    except Exception:
        actions = None
    # the Atspi GObject may only expose the snake_case interface
    if actions is None:
        try:
            if obj.get_n_actions() > 0:
                obj.do_action(0)
                return True
        except Exception:
            pass
        return False
    for i in range(actions.nActions):
        try:
            name = actions.getName(i)
        except Exception:
            name = ""
        if name.lower() in ("toggle", "click", "press"):
            actions.doAction(i)
            return True
    return False


def click_at(rect, display=":99", left_edge=False):
    """Synthesize a left click via XTEST at a11y-reported coordinates.
    The Qt a11y bridge exposes NO working actions in this stack (verified:
    the cell Toggle action does not change the checkbox and the Execute
    button has zero actions), so real (synthesized) pointer events at
    a11y-derived coordinates are the activation path. `left_edge` targets
    the checkbox INDICATOR area (the left of the cell), where Qt toggles
    on click; a center click on the cell only selects."""
    try:
        x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
        xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
        x11.XOpenDisplay.restype = ctypes.c_void_p
        dpy = x11.XOpenDisplay(display.encode())
        if not dpy:
            return False
        if left_edge:
            cx = int(rect.x) + max(6, int(rect.width) // 4)
        else:
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


def click_execute(app):
    """Find the Execute push button and activate it (action if exposed,
    else a11y-coordinate XTEST click)."""
    stack = [app]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
            nm = (node.name or "").lower()
        except Exception:
            r, nm = None, ""
        if r in ("PUSH_BUTTON", "43") and "execute" in nm:
            if toggle_via_action(node):
                return True
            try:
                comp = node.get_component()
                if comp is not None:
                    rect = comp.get_extents(0)
                    return click_at(rect)
            except Exception as e:
                print(f"WARN: execute button click failed: {e}", file=sys.stderr)
                return False
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return False


def count_pacman_execs(trace_path):
    try:
        with open(trace_path, "r", errors="replace") as f:
            return sum(1 for line in f if "execve" in line and '"pacman"' in line)
    except OSError:
        return 0


def main():
    out_state = sys.argv[1]
    targets = sys.argv[2:]
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
    tree = find_tree(app, targets[0] if targets else "")
    if tree is None:
        print("FATAL: kernels tree not found", file=sys.stderr)
        with open(out_state, "w", encoding="utf-8") as f:
            json.dump(
                {
                    "schema": "cachyos-km-oracle-a11y-v1",
                    "observable": "full-at-spi-tree",
                    "app_name": app.name or "",
                    "rows_populated": ok,
                    "tree": dump_node(app),
                    "driver_error": "kernels tree not found",
                },
                f,
                indent=1,
                ensure_ascii=False,
            )
        return 1

    # dump the DISCOVERY tree BEFORE any toggle: the row comparison courts
    # the default checkbox state (installed+immutable -> checked), and the
    # post-toggle tree would carry the transaction's selection instead.
    payload = {
        "schema": "cachyos-km-oracle-a11y-v1",
        "observable": "full-at-spi-tree",
        "app_name": app.name or "",
        "rows_populated": ok,
        "tree": dump_node(app),
    }
    with open(out_state, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=1, ensure_ascii=False)
    print(f"discovery tree dumped to {out_state}")

    for target in targets:
        cell = find_cell_by_text(tree, target)
        if cell is None:
            print(f"FATAL: row {target!r} not found in the tree", file=sys.stderr)
            with open(out_state, "w", encoding="utf-8") as f:
                json.dump(
                    {
                        "schema": "cachyos-km-oracle-a11y-v1",
                        "observable": "full-at-spi-tree",
                        "app_name": app.name or "",
                        "rows_populated": ok,
                        "tree": dump_node(app),
                        "driver_error": f"row {target!r} not found",
                    },
                    f,
                    indent=1,
                    ensure_ascii=False,
                )
            return 1
        if not toggle_row_click(cell):
            print(f"FATAL: could not toggle row {target!r}", file=sys.stderr)
            return 1
        time.sleep(0.5)

    if not click_execute(app):
        print("FATAL: Execute button not found/clicked", file=sys.stderr)
        return 1
    print("execute clicked")

    # wait for the transaction to actually start (pacman execve in trace)
    deadline = time.time() + TIMEOUT
    seen = 0
    while time.time() < deadline:
        seen = count_pacman_execs(trace_path)
        if seen >= 1:
            break
        time.sleep(POLL)
    with open(marker, "w") as f:
        json.dump({"pacman_execs": seen, "targets": targets}, f)
    print(f"transaction started: pacman execs seen = {seen}")

    # a short settle so the strace buffer flushes before extraction
    time.sleep(2)
    return 0 if seen >= 1 else 2


if __name__ == "__main__":
    sys.exit(main())
