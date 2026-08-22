#!/usr/bin/env python3
"""oracle-mutate.py — drive the REAL oracle Configure window via AT-SPI.

Used by the patch-injection and custom-name courts (Phase 6). After the
standard Configure click (which opens the conf window after the background
prepare_build_environment), it:

1. waits for the conf window (a WINDOW containing the "Build kernel" button),
2. sets the custom package name (a11y editable text, XTEST fallback),
3. adds a remote patch through the QInputDialog (entry text + OK),
4. captures the PKGBUILD BEFORE the Build click,
5. clicks "Build kernel" (on_execute: source-array splice + pkgbase insert),
6. captures the PKGBUILD AFTER.

The oracle is launched with cwd = /root/.cache/cachyos-km/pkgbuilds, so the
oracle's relative `linux-cachyos/PKGBUILD` paths (conf-window.cpp:124-148,
204-339) resolve to the seeded checkout (D-004 design assumption).

Raw evidence discipline: only the tree dump + the two PKGBUILD snapshots +
a marker are written; the mutations themselves are the compared observables.
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

# char -> X11 keysym name (for the XTEST fallback typer)
CHAR_KEYSYM = {
    " ": "space", "-": "minus", "_": "underscore", ":": "colon", "/": "slash",
    ".": "period", ",": "comma", ";": "semicolon", "'": "apostrophe",
    '"': "quotedbl", "`": "grave", "~": "asciitilde", "!": "exclam",
    "@": "at", "#": "numbersign", "$": "dollar", "%": "percent",
    "^": "asciicircum", "&": "ampersand", "*": "asterisk", "(": "parenleft",
    ")": "parenright", "[": "bracketleft", "]": "bracketright",
    "{": "braceleft", "}": "braceright", "\\": "backslash", "|": "bar",
    "<": "less", ">": "greater", "=": "equal", "+": "plus", "?": "question",
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
        comp = obj.get_component()
        if comp is not None:
            ex = comp.get_extents(0)
            node["extents"] = [ex.x, ex.y, ex.width, ex.height]
    except Exception:
        pass
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


def type_text(text, display=":99"):
    """Type `text` via XTEST key events. For each char, resolve the keysym,
    find its keycode, and press shift iff the keycode's UNshifted keysym
    differs from the target keysym (handles ':' etc.)."""
    try:
        x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
        xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XStringToKeysym.restype = ctypes.c_ulong
        x11.XKeysymToKeycode.restype = ctypes.c_ubyte
        x11.XGetKeyboardMapping.restype = ctypes.POINTER(ctypes.c_ulong)
        dpy = x11.XOpenDisplay(display.encode())
        if not dpy:
            return False
        for ch in text:
            name = CHAR_KEYSYM.get(ch, ch if ch.isalnum() else None)
            if name is None:
                print(f"WARN: no keysym for {ch!r}", file=sys.stderr)
                continue
            ks = x11.XStringToKeysym(name.encode())
            if ks == 0:
                print(f"WARN: no keysym for name {name!r}", file=sys.stderr)
                continue
            kc = x11.XKeysymToKeycode(dpy, ctypes.c_ulong(ks))
            if kc == 0:
                continue
            n_keysyms = ctypes.c_int()
            mapping = x11.XGetKeyboardMapping(dpy, ctypes.c_ubyte(kc), 1, ctypes.byref(n_keysyms))
            unshifted = mapping[0]
            shift = 1 if unshifted != ks else 0
            x11.XTestFakeKeyEvent.restype = ctypes.c_int
            if shift:
                shift_kc = x11.XKeysymToKeycode(dpy, x11.XStringToKeysym(b"Shift_L"))
                xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(shift_kc), 1, 0)
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(kc), 1, 0)
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(kc), 0, 0)
            if shift:
                xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(shift_kc), 0, 0)
            x11.XFlush(ctypes.c_void_p(dpy))
            time.sleep(0.02)
        return True
    except Exception as e:
        print(f"WARN: XTEST type failed: {e}", file=sys.stderr)
        return False


def set_entry_text(entry, text, display=":99"):
    """Prefer the AT-SPI editable-text interface; fall back to XTEST (click
    to focus, select-all, type)."""
    try:
        editable = entry.queryEditableText()
        editable.setTextContents(text)
        try:
            got = entry.queryText().getText(0, -1)
        except Exception:
            got = ""
        if got == text:
            return True
    except Exception:
        pass
    try:
        comp = entry.get_component()
        if comp is not None and click_at(comp.get_extents(0), display):
            time.sleep(0.3)
            # select all (Ctrl+A) then type
            x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
            xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
            x11.XOpenDisplay.restype = ctypes.c_void_p
            dpy = x11.XOpenDisplay(display.encode())
            ctrl_kc = x11.XKeysymToKeycode(dpy, x11.XStringToKeysym(b"Control_L"))
            a_kc = x11.XKeysymToKeycode(dpy, x11.XStringToKeysym(b"a"))
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(ctrl_kc), 1, 0)
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(a_kc), 1, 0)
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(a_kc), 0, 0)
            xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), ctypes.c_uint(ctrl_kc), 0, 0)
            x11.XFlush(ctypes.c_void_p(dpy))
            time.sleep(0.2)
            return type_text(text, display)
    except Exception as e:
        print(f"WARN: XTEST entry fallback failed: {e}", file=sys.stderr)
    return False


def click_tab(root, needle):
    """Activate a page tab whose name contains `needle` (the conf window's
    Options/Patches tabs). Controls on a hidden tab have garbage extents, so
    the tab must be switched BEFORE clicking anything on it."""
    stack = [root]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
            nm = (node.name or "").lower()
        except Exception:
            continue
        if r in ("PAGE_TAB", "37") and needle in nm:
            return click_button(node)
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return False


def find_button(root, needle):
    """Find a PUSH_BUTTON whose name/text contains `needle` (lowercase)."""
    stack = [root]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
            nm = (node.name or "").lower()
            if r in ("PUSH_BUTTON", "43") and needle in nm:
                return node
        except Exception:
            pass
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return None


def find_entry(root, needle=None):
    """Find a text-input control (Qt exposes QLineEdit variously as ENTRY,
    TEXT or EDITBAR under the a11y bridge); if `needle` is given, one whose
    text/name contains it."""
    stack = [root]
    while stack:
        node = stack.pop()
        try:
            r = role_name(node.getRole())
        except Exception:
            continue
        if r in ("ENTRY", "79", "TEXT", "61", "EDITBAR", "77"):
            txt = ""
            try:
                txt = node.queryText().getText(0, -1)
            except Exception:
                txt = node.name or ""
            if needle is None or needle.lower() in (txt or "").lower():
                return node
        try:
            stack.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return None


def click_button(node, display=":99"):
    """Activate a push button. Phase 5 verified that the Qt a11y bridge's
    exposed actions are unreliable (Toggle on cells does nothing), while
    REAL synthesized pointer events at a11y coordinates work — so XTEST
    comes first; the action interface is only a fallback."""
    try:
        comp = node.get_component()
        if comp is not None and click_at(comp.get_extents(0), display):
            return True
    except Exception:
        pass
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
    return False


def is_showing(node):
    try:
        states = node.getState().getStates()
        names = {getattr(s, "name", str(s)).lower() for s in states}
        return "showing" in names or "visible" in names
    except Exception:
        return True


def wait_for_window(needle, showing=True):
    """Wait for a window (anywhere on the desktop — the ConfWindow is a
    separate top-level QMainWindow) containing a button named `needle`.
    When `showing`, hidden windows (the a11y bridge exposes pre-show
    windows) are skipped — clicks on hidden controls go nowhere."""
    deadline = time.time() + TIMEOUT
    while time.time() < deadline:
        desktop = pyatspi.Registry.getDesktop(0)
        stack = [desktop]
        while stack:
            node = stack.pop()
            try:
                r = role_name(node.getRole())
            except Exception:
                continue
            if r in ("WINDOW", "69", "DIALOG", "16", "FRAME", "23"):
                if (not showing or is_showing(node)) and find_button(node, needle) is not None:
                    return node
            try:
                stack.extend(node[i] for i in range(node.childCount))
            except Exception:
                pass
        time.sleep(POLL)
    return None


def dump_x_windows(display=":99"):
    """XQueryTree the root window: real stacking order + geometry (no WM
    under Xvfb, so a11y extents can lie about which window is on top).
    Fully defensive — a crash here must never kill the driver."""
    try:
        import ctypes as _c
        x11 = _c.cdll.LoadLibrary(_c.util.find_library("X11"))
        x11.XOpenDisplay.restype = _c.c_void_p
        x11.XDefaultRootWindow.restype = _c.c_ulong
        x11.XQueryTree.restype = _c.c_int
        x11.XQueryTree.argtypes = [_c.c_void_p, _c.c_ulong,
                                   _c.POINTER(_c.c_ulong), _c.POINTER(_c.c_ulong),
                                   _c.POINTER(_c.POINTER(_c.c_ulong)), _c.POINTER(_c.c_uint)]
        x11.XGetGeometry.restype = _c.c_int
        x11.XGetGeometry.argtypes = [_c.c_void_p, _c.c_ulong,
                                     _c.POINTER(_c.c_ulong), _c.POINTER(_c.c_int),
                                     _c.POINTER(_c.c_int), _c.POINTER(_c.c_uint),
                                     _c.POINTER(_c.c_uint), _c.POINTER(_c.c_uint), _c.POINTER(_c.c_uint)]
        x11.XFetchName.restype = _c.c_int
        x11.XFetchName.argtypes = [_c.c_void_p, _c.c_ulong, _c.POINTER(_c.c_char_p)]
        x11.XFree.restype = _c.c_int
        x11.XFree.argtypes = [_c.c_void_p]
        dpy = x11.XOpenDisplay(display.encode())
        if not dpy:
            return []
        root = x11.XDefaultRootWindow(dpy)
        rroot = _c.c_ulong()
        rparent = _c.c_ulong()
        children = _c.POINTER(_c.c_ulong)()
        n = _c.c_uint()
        if not x11.XQueryTree(dpy, root, _c.byref(rroot), _c.byref(rparent),
                              _c.byref(children), _c.byref(n)):
            return []
        out = []
        for i in range(n.value):
            w = children[i]
            junk = _c.c_ulong()
            x = _c.c_int(); y = _c.c_int()
            wd = _c.c_uint(); h = _c.c_uint(); bd = _c.c_uint(); dep = _c.c_uint()
            x11.XGetGeometry(dpy, w, _c.byref(junk), _c.byref(x), _c.byref(y),
                             _c.byref(wd), _c.byref(h), _c.byref(bd), _c.byref(dep))
            name = _c.c_char_p()
            try:
                x11.XFetchName(dpy, w, _c.byref(name))
                nm = (name.value or b"").decode(errors="replace")
            except Exception:
                nm = ""
            out.append({"win": int(w), "name": nm,
                        "x": x.value, "y": y.value, "w": wd.value, "h": h.value})
        x11.XFree(_c.cast(children, _c.c_void_p))
        return out  # root child order = bottom-to-top stacking
    except Exception as e:
        print(f"WARN: X window dump failed: {e}", file=sys.stderr)
        return []


def dump_desktop(tag):
    """Dump the whole desktop a11y tree to /tmp/conf-debug-<tag>.json
    (failure diagnostics that stay as evidence)."""
    try:
        desktop = pyatspi.Registry.getDesktop(0)
        dbg = {
            "tag": tag,
            "desktop": [dump_node(desktop[i]) for i in range(desktop.childCount)],
        }
        with open(f"/tmp/conf-debug-{tag}.json", "w", encoding="utf-8") as f:
            json.dump(dbg, f, indent=1, ensure_ascii=False)
        return True
    except Exception as e:
        print(f"WARN: debug dump {tag} failed: {e}", file=sys.stderr)
        return False


def main():
    out_state = sys.argv[1] if len(sys.argv) > 1 else "/tmp/oracle-state.json"
    custom_name = None
    patch_url = None
    args = sys.argv[2:]
    i = 0
    while i < len(args):
        if args[i] == "--custom-name" and i + 1 < len(args):
            custom_name = args[i + 1] or None
            i += 2
        elif args[i] == "--patch-url" and i + 1 < len(args):
            patch_url = args[i + 1] or None
            i += 2
        else:
            i += 1

    PKGBUILD = "/root/.cache/cachyos-km/pkgbuilds/linux-cachyos/PKGBUILD"
    marker = "/tmp/oracle-mutate.marker"

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
    tree = {"schema": "cachyos-km-oracle-a11y-v1", "observable": "full-at-spi-tree",
            "app_name": app.name or "", "rows_ready": ok, "children": [dump_node(app)]}
    with open(out_state, "w", encoding="utf-8") as f:
        json.dump(tree, f, indent=1, ensure_ascii=False)
    print(f"dumped a11y tree (rows_ready={ok})")

    # open the conf window
    configure = find_button(app, "configure")
    if configure is None:
        print("FATAL: Configure button not found", file=sys.stderr)
        return 1
    if not click_button(configure):
        print("FATAL: Configure click failed", file=sys.stderr)
        return 2
    print("configure clicked")

    conf = wait_for_window("build kernel")
    if conf is None:
        print("FATAL: conf window not found (Build kernel button)", file=sys.stderr)
        dump_desktop("conf-window")
        return 3
    print("conf window open")

    # NOTE: find_button can match HIDDEN windows (the a11y bridge exposes
    # them); prefer the SHOWING conf window for all subsequent lookups.
    conf_visible = find_button(conf, "build kernel") is not None

    # custom name (the entry's default text is $pkgbase-custom)
    if custom_name is not None:
        entry = find_entry(conf, "$pkgbase-custom")
        if entry is None:
            # fall back to any entry under the custom-name label
            entry = find_entry(conf)
        if entry is None:
            print("FATAL: custom-name entry not found", file=sys.stderr)
            return 4
        if not set_entry_text(entry, custom_name):
            print("WARN: custom-name set may have failed", file=sys.stderr)

    # remote patch via the QInputDialog (the Patches TAB must be current:
    # controls on a hidden tab report garbage extents)
    if patch_url is not None:
        if not click_tab(conf, "patches"):
            print("WARN: could not switch to the Patches tab", file=sys.stderr)
        time.sleep(0.5)
        add_btn = find_button(conf, "add remote patch")
        if add_btn is None:
            print("FATAL: Add remote patch button not found", file=sys.stderr)
            return 5
        if not click_button(add_btn):
            print("FATAL: Add remote patch click failed", file=sys.stderr)
            return 5
        print("add-remote-patch clicked")
        dlg = wait_for_window("ok")
        if dlg is None:
            print("FATAL: QInputDialog not found", file=sys.stderr)
            dump_desktop("input-dialog")
            return 6
        entry = find_entry(dlg)
        if entry is None:
            print("FATAL: QInputDialog entry not found", file=sys.stderr)
            dump_desktop("input-dialog-entry")
            return 6
        if not set_entry_text(entry, patch_url):
            print("WARN: patch URL set may have failed", file=sys.stderr)
        ok_btn = find_button(dlg, "ok")
        if ok_btn is None:
            print("FATAL: QInputDialog OK not found", file=sys.stderr)
            return 6
        click_button(ok_btn)
        # the dialog must actually CLOSE (a missed OK leaves the modal
        # dialog open and every subsequent click is swallowed by it)
        deadline = time.time() + 20
        while time.time() < deadline and wait_for_window("ok", showing=False) is not None:
            time.sleep(POLL)
        if wait_for_window("ok", showing=False) is not None:
            print("WARN: QInputDialog did not close after OK", file=sys.stderr)
            dump_desktop("dialog-still-open")
        print("patch URL entered + OK (dialog closed)")

    # the button bar (Load/Save/Cancel/Build kernel) lives on the OPTIONS
    # page (conf-options-page.ui footer_widget) — switch back from the
    # Patches tab or the buttons report zero extents
    click_tab(conf, "options")
    time.sleep(0.5)

    # snapshot BEFORE the build click
    try:
        with open(PKGBUILD, "r") as f:
            before = f.read()
    except OSError as e:
        print(f"FATAL: cannot read PKGBUILD before: {e}", file=sys.stderr)
        return 7
    with open("/tmp/pkgbuild-before.txt", "w") as f:
        f.write(before)
    print(f"pkgbuild-before captured ({len(before)} bytes)")

    # the Build kernel click (on_execute: splice + pkgbase insert). Retry:
    # a click can land on the wrong Z-layer under Xvfb (no window manager).
    def pkgbuild_text():
        try:
            with open(PKGBUILD, "r") as f:
                return f.read()
        except OSError:
            return None

    mutated = False
    for attempt in range(6):
        build_btn = find_button(conf, "build kernel")
        if build_btn is None:
            print("FATAL: Build kernel button not found", file=sys.stderr)
            return 8
        # try the AT-SPI action interface first (no Z-order ambiguity), then
        # a real XTEST pointer event; verify via the PKGBUILD mutation
        action_ok = False
        try:
            acts = build_btn.getAction()
            if acts is not None and acts.nActions > 0:
                acts.doAction(0)
                action_ok = True
        except Exception:
            pass
        time.sleep(1.5)
        now = pkgbuild_text()
        if now is not None and now != before:
            mutated = True
            print(f"build kernel activated (attempt {attempt + 1}{' action' if action_ok else ' click'}) -> mutation detected")
            break
        click_button(build_btn)
        time.sleep(1.5)
        now = pkgbuild_text()
        if now is not None and now != before:
            mutated = True
            print(f"build kernel clicked (attempt {attempt + 1}) -> mutation detected")
            break
        print(f"build kernel activation attempt {attempt + 1}: no mutation yet", file=sys.stderr)
    if not mutated:
        print("FATAL: Build kernel activation did not mutate the PKGBUILD", file=sys.stderr)
        dump_desktop("build-click")
        return 8

    # give the mutation a moment, then snapshot AFTER
    time.sleep(2.0)
    try:
        with open(PKGBUILD, "r") as f:
            after = f.read()
    except OSError as e:
        print(f"FATAL: cannot read PKGBUILD after: {e}", file=sys.stderr)
        return 7
    with open("/tmp/pkgbuild-after.txt", "w") as f:
        f.write(after)
    print(f"pkgbuild-after captured ({len(after)} bytes)")

    with open(marker, "w") as f:
        f.write(f"custom_name={custom_name}\npatch_url={patch_url}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
