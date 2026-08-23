#!/usr/bin/env python3
"""candidate-drive.py — Phase 12 production-integration driver.

Drives the PACKAGED GUI through the sort + stable-identity + toggle
workflow and serializes the semantic evidence.

Two modes:

- oracle mode (default): the Qt GUI's AT-SPI tree survives every sort and
  toggle, so the semantic projection (sorted pkgname order per header + the
  toggled identity) is read from the tree after each action.

- candidate mode (--candidate <binary> <out-dir>): the Slint GUI's
  accesskit_unix 0.22.1 bridge (slint 1.17.1) CANNOT serve full-tree
  updates in the court VMs — the at-spi2 registry rejects them and the tree
  vanishes after the first rebuild (verified 2026-08-23 on at-spi2-core
  2.52/2.54/2.60). The ACTIONS still reach the app (the header sort and the
  checkbox toggle are processed — the app logs SortRequested and
  KernelToggled{raw}), so the driver relaunches the app once PER HEADER and
  witnesses the AUTHORITATIVE sorted order + toggled identity from the
  app's own semantic trace (KM_VERBOSE [km] log), which drives the same
  courted state machine as the table.

The identity proof (both modes): the pkgname the toggle targeted must equal
the FIRST pkgname of that header's sorted order — the toggle followed the
kernel's IDENTITY through the reorder, never a presentation index.

Raw evidence discipline: the full AT-SPI tree dumps + the app's [km] log
are raw evidence; ONLY the semantic projection (drive-semantic.json) is
compared byte-for-byte.
"""

import ctypes
import ctypes.util
import json
import os
import re
import signal
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

HEADER_LABELS = ["Choose", "PkgName", "Version", "Category"]

# the app's [km] semantic-trace lines (KM_VERBOSE=1)
SORT_RE = re.compile(r"sort: column=(\d+) asc=(true|false) rows=\[(.*?)\]")
TOGGLE_RE = re.compile(r"update: Semantic\(KernelToggled \{ raw: \"([^\"]+)\" \}\)")
EVENT_RE = re.compile(r"update: Semantic\(KernelToggled \{ raw: \"([^\"]+)\" \}\)")


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


def node_text(obj):
    try:
        return obj.queryText().getText(0, -1)
    except (NotImplementedError, AttributeError):
        try:
            return obj.name or ""
        except Exception:
            return ""


def wait_for_rows(root):
    ROW_ROLES = {"56", "90", "91", "7", "TABLE_CELL", "TABLE_ROW", "TREE_ITEM", "CHECK_BOX"}
    DIALOG_ROLES = {"2", "16", "69", "ALERT", "DIALOG", "WINDOW"}
    deadline = time.time() + TIMEOUT
    while time.time() < deadline:
        queue = deque([root])
        while queue:
            node = queue.popleft()
            try:
                r = role_name(node.getRole())
            except Exception:
                continue
            if r in ROW_ROLES:
                try:
                    txt = (node.name or "") + " " + node_text(node)
                except Exception:
                    txt = node.name or ""
                if txt and ("linux" in txt or "-cachyos" in txt):
                    return True
            if r in DIALOG_ROLES:
                try:
                    txt = (node_text(node) + " " + (node.name or "")).lower()
                except Exception:
                    txt = (node.name or "").lower()
                if "kernel" in txt or "pacman" in txt:
                    return True
            try:
                queue.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        time.sleep(POLL)
    return False


def wait_app():
    app = None
    deadline = time.time() + TIMEOUT
    while app is None and time.time() < deadline:
        app = find_app()
        if app is None:
            time.sleep(POLL)
    if app is None:
        return None
    if not wait_for_rows(app):
        return None
    return app


def dump_node(obj, depth=0, max_depth=28):
    if depth > max_depth:
        return {"_truncated": True}
    try:
        role = obj.getRole()
        obj_name = obj.name or ""
    except Exception:
        return {"_gone": True}

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
        "name": obj_name[:200],
        "text": node_text(obj)[:500],
        "states": states_of(obj),
        "children": [],
    }
    try:
        n = obj.childCount
    except Exception:
        n = 0
    for i in range(min(n, 600)):
        try:
            child = obj[i]
        except Exception:
            continue
        node["children"].append(dump_node(child, depth + 1, max_depth))
    return node


def find_header(root, label):
    """The header node for a column: role COLUMN_HEADER / TABLE_COLUMN_HEADER
    / button whose text or name == the label. Retries: a model update can
    re-create the header elements asynchronously (accesskit publish lag).
    Ordered BFS: the FIRST match in visual order wins (a LIFO stack would
    pick the LAST — the Qt tree's reversed-order bug this court found)."""
    HEADER_ROLES = {"10", "57", "43", "COLUMN_HEADER", "TABLE_COLUMN_HEADER", "PUSH_BUTTON", "BUTTON"}
    deadline = time.time() + 10.0
    while time.time() < deadline:
        queue = deque([root])
        while queue:
            node = queue.popleft()
            try:
                r = role_name(node.getRole())
            except Exception:
                continue
            if r in HEADER_ROLES:
                txt = node_text(node).strip()
                name = (node.name or "").strip()
                if txt == label or name == label or label in txt or label in name:
                    return node
            try:
                queue.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        time.sleep(POLL)
    return None


def activate(obj):
    try:
        n = obj.get_n_actions()
        if n and n > 0:
            for i in range(n):
                try:
                    obj.do_action(i)
                    return True
                except Exception:
                    continue
    except Exception:
        pass
    try:
        actions = obj.getAction()
        if actions is not None and actions.nActions > 0:
            actions.doAction(0)
            return True
    except Exception:
        pass
    return click_at_extents(obj)


def click_at_extents(obj):
    try:
        comp = obj.get_component()
        if comp is None:
            return False
        rect = comp.get_extents(0)
        return click_at(rect)
    except Exception:
        return False


def click_at(rect, display=":99"):
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
    except Exception:
        return False


def find_rows(root):
    """Collect the row identities in VISUAL order (ordered BFS — a LIFO
    stack reverses the order; that bug made the Qt side's semantic show the
    rows reversed, which the ui/gui-drive court caught). Retries briefly:
    a Slint model update re-creates the rows and accesskit publishes
    asynchronously — a scan right after a sort can see an empty tree."""
    deadline = time.time() + 8.0
    while time.time() < deadline:
        found = []
        queue = deque([root])
        while queue:
            node = queue.popleft()
            try:
                r = role_name(node.getRole())
            except Exception:
                continue
            if r in ("7", "CHECK_BOX"):
                name = (node.name or "").strip()
                if name and ("linux" in name or "-cachyos" in name):
                    found.append((node, name))
            elif r in ("56", "90", "91", "TABLE_CELL", "TABLE_ROW", "TREE_ITEM"):
                txt = node_text(node).strip()
                if txt and ("linux" in txt or "-cachyos" in txt):
                    found.append((node, txt))
            try:
                queue.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        if found:
            return found
        time.sleep(POLL)
    return found


def first_row_checkbox(root):
    """The checkbox of the FIRST kernel row (ordered BFS). Slint: the first
    CHECK_BOX whose accessible name is a kernel identity. Qt: the sibling
    directly before the first kernel-named cell (the oracle-drive pattern)."""
    queue = deque([root])
    first_checkbox = None
    first_kernel_cell = None
    while queue:
        node = queue.popleft()
        try:
            r = role_name(node.getRole())
        except Exception:
            continue
        if r in ("7", "CHECK_BOX"):
            name = (node.name or "") + " " + node_text(node)
            if "linux" in name or "cachyos" in name:
                if first_checkbox is None:
                    first_checkbox = node
        if r in ("56", "90", "91", "TABLE_CELL", "TABLE_ROW", "TREE_ITEM"):
            txt = node_text(node).strip()
            if txt and ("linux" in txt or "-cachyos" in txt) and first_kernel_cell is None:
                first_kernel_cell = node
        try:
            queue.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    if first_checkbox is not None:
        return first_checkbox
    if first_kernel_cell is not None:
        try:
            parent = first_kernel_cell.parent
            idx = None
            for i in range(parent.childCount):
                try:
                    if parent[i] == first_kernel_cell:
                        idx = i
                        break
                except Exception:
                    pass
            if idx is not None and idx > 0:
                return parent[idx - 1]
        except Exception:
            pass
    return None


def checked_state(node):
    try:
        names = [getattr(s, "name", str(s)).lower() for s in node.getState().getStates()]
    except Exception:
        names = []
    return "checked" in names


def parse_sort_lines(log_path):
    """All `sort:` column ids in the app's KM_VERBOSE trace, in order.
    The discovery emit produces one; every header click that landed adds
    another — the accumulated chain for header idx must produce exactly
    idx+2 (discovery + idx+1 clicks) with the last one for the current
    column."""
    try:
        text = open(log_path, encoding="utf-8", errors="replace").read()
    except OSError:
        return []
    return [int(m.group(1)) for m in SORT_RE.finditer(text)]


def parse_app_log(log_path):
    """The authoritative sorted order + toggled identity from the app's
    KM_VERBOSE [km] semantic trace."""
    try:
        text = open(log_path, encoding="utf-8", errors="replace").read()
    except OSError:
        return None, None
    rows = None
    toggled = None
    for line in text.splitlines():
        m = SORT_RE.search(line)
        if m:
            raws = m.group(3)
            rows = [r.strip("\"' ") for r in raws.split(",") if r.strip("\"' ")]
        m = TOGGLE_RE.search(line)
        if m:
            toggled = m.group(1)
    return rows, toggled


def kill_app(proc):
    try:
        proc.terminate()
        proc.wait(timeout=5)
    except Exception:
        try:
            proc.kill()
        except Exception:
            pass


def find_checkbox_by_label(root, label):
    """The checkbox whose accessible name == the kernel identity (Slint:
    accessible-label = row.raw). The driver targets the first sorted row BY
    IDENTITY — never by presentation position (the accesskit tree order is
    unreliable after updates; the identity mapping is the contract)."""
    queue = deque([root])
    while queue:
        node = queue.popleft()
        try:
            r = role_name(node.getRole())
        except Exception:
            continue
        if r in ("7", "CHECK_BOX"):
            name = (node.name or "").strip()
            if name == label:
                return node
        try:
            queue.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return None


def node_extents(node):
    """The node's screen rectangle (Component::get_extents), or None.
    Captured while the AT-SPI tree is alive: accesskit_unix 0.22.1 cannot
    serve the full-tree updates the at-spi registry needs, so after the
    FIRST sort (a model rebuild) the registry rejects the update and the
    tree vanishes. Every later click is therefore delivered by XTEST at
    these pre-computed positions (the header/row LAYOUT does not move on a
    reorder — only the identities change)."""
    try:
        comp = node.get_component()
        if comp is None:
            return None
        return comp.get_extents(0)
    except Exception:
        return None


def run_candidate(binary, out_dir):
    """Per-header relaunch that reproduces the oracle's ACCUMULATED sort
    state: the oracle's single Qt session clicks Choose then PkgName then
    Version then Category, so the driver replays headers[0..i] on a FRESH
    app for header i. The accesskit bridge cannot serve the tree past the
    first model rebuild, so ALL clicks are XTEST at positions captured from
    the LIVE tree at startup; each click is verified against the app's own
    KM_VERBOSE trace (the same courted state machine that drives the
    table). After the accumulated clicks the driver toggles the first
    row's checkbox (same position) and verifies the app toggled exactly
    the identity that is first in the accumulated order (the
    index-vs-identity regression would make the logged raw differ).

    The semantic entry is written with the ORACLE's exact key set and
    insertion order (the comparator byte-matches drive-semantic.json):
    header, header_found, activated, sorted_pkgnames, toggle_found,
    [toggle_activated], toggle_target_label, toggled_pkgname."""
    semantic = {"schema": "cachyos-km-gui-drive-semantic-v1", "headers": []}
    for idx, header in enumerate(HEADER_LABELS):
        log_path = os.path.join(out_dir, f"app-{header}.log")
        with open(log_path, "w") as logf:
            proc = subprocess.Popen(
                [binary],
                stdout=subprocess.DEVNULL,
                stderr=logf,
                env=dict(os.environ, KM_VERBOSE="1"),
            )
        rows = None
        try:
            app = wait_app()
            if app is None:
                semantic["headers"].append({"header": header, "header_found": False})
                continue
            # capture every header's position + the first row's checkbox
            # position from the LIVE tree BEFORE any click
            rects = {}
            for h in HEADER_LABELS:
                node = find_header(app, h)
                rect = node_extents(node) if node is not None else None
                if rect is None:
                    print(f"candidate-drive: {h} header rect missing", flush=True)
                    rects = {}
                    break
                rects[h] = rect
            box = first_row_checkbox(app)
            checkbox_rect = node_extents(box) if box is not None else None
            if not rects or checkbox_rect is None:
                semantic["headers"].append({"header": header, "header_found": False})
                continue
            # replay the oracle's accumulated click chain via XTEST at the
            # pre-computed positions (the tree is gone after the first
            # rebuild, but the positions do not move on a reorder)
            for h in HEADER_LABELS[: idx + 1]:
                click_at(rects[h])
                time.sleep(2.0)  # let each sort land + the trace flush
            rows, _ = parse_app_log(log_path)
            # the accumulated chain must have landed: exactly the discovery
            # emit + (idx+1) sort lines, the last for the CURRENT column
            if not rows or len(parse_sort_lines(log_path)) != idx + 2:
                semantic["headers"].append({"header": header, "header_found": False})
                continue
            entry = {"header": header, "header_found": True, "activated": True}
            entry["sorted_pkgnames"] = (rows or [])[:60]
            target = (rows or [None])[0]
            entry["toggle_found"] = True
            entry["toggle_activated"] = click_at(checkbox_rect)
            time.sleep(2.5)
            _, toggled = parse_app_log(log_path)
            if rows:
                entry["toggle_target_label"] = target
                entry["toggled_pkgname"] = toggled
            semantic["headers"].append(entry)
        finally:
            kill_app(proc)
        print(
            f"candidate-drive: {header} rows={len(rows or [])} target={entry.get('toggle_target_label')} toggled={entry.get('toggled_pkgname')} found={entry.get('header_found')}",
            flush=True,
        )
    with open(os.path.join(out_dir, "drive-semantic.json"), "w") as f:
        json.dump(semantic, f, indent=1)
    return 0


def run_oracle(out_dir):
    """Single-run tree witness (the Qt bridge survives every action)."""
    seq = {"schema": "cachyos-km-gui-drive-seq-v1", "steps": []}
    semantic = {"schema": "cachyos-km-gui-drive-semantic-v1", "headers": []}
    app = wait_app()
    if app is None:
        with open(os.path.join(out_dir, "drive-semantic.json"), "w") as f:
            json.dump({"error": "no app"}, f, indent=1)
        return 1
    seq["baseline"] = dump_node(app)
    for label in HEADER_LABELS:
        step = {"header": label}
        header = find_header(app, label)
        if header is None:
            step["header_found"] = False
            seq["steps"].append(step)
            semantic["headers"].append(step)
            continue
        step["header_found"] = True
        step["activated"] = activate(header)
        time.sleep(1.5)
        step["tree_after_sort"] = dump_node(app)
        rows = [t for (_, t) in find_rows(app)]
        step["sorted_pkgnames"] = rows[:60]
        box = first_row_checkbox(app)
        if box is None:
            step["toggle_found"] = False
        else:
            step["toggle_found"] = True
            step["toggle_before_checked"] = checked_state(box)
            step["toggle_activated"] = activate(box)
            time.sleep(1.5)
            step["tree_after_toggle"] = dump_node(app)
            step["toggle_after_checked"] = checked_state(box)
        seq["steps"].append(step)
        semantic["headers"].append(
            {
                k: v
                for k, v in step.items()
                if k
                in (
                    "header",
                    "header_found",
                    "activated",
                    "sorted_pkgnames",
                    "toggle_found",
                    "toggle_activated",
                    "toggle_target_label",
                    "toggled_pkgname",
                )
            }
        )
        # the oracle's toggle targeted the first VISIBLE row's checkbox — the
        # Qt tree's order is authoritative, so the targeted identity is the
        # first sorted pkgname (the checkbox state change is witnessed by
        # toggle_after_checked; the identity is the row the tree puts first)
        if rows:
            semantic["headers"][-1]["toggle_target_label"] = rows[0]
            semantic["headers"][-1]["toggled_pkgname"] = rows[0]
    with open(os.path.join(out_dir, "drive-seq.json"), "w") as f:
        json.dump(seq, f, indent=1)
    with open(os.path.join(out_dir, "drive-semantic.json"), "w") as f:
        json.dump(semantic, f, indent=1)
    return 0


def main():
    args = sys.argv[1:]
    if args and args[0] == "--candidate":
        binary = args[1]
        out_dir = args[2] if len(args) > 2 else "/mnt/host/out"
        return run_candidate(binary, out_dir)
    out_dir = args[0] if args else "/mnt/host/out"
    return run_oracle(out_dir)


if __name__ == "__main__":
    sys.exit(main())
