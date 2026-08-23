#!/usr/bin/env python3
"""candidate-transact.py — toggle a kernel row + click Execute on the
release Slint binary (fresh app, no prior sort: the accesskit bridge
survives ONE model rebuild — the toggle — and the Execute button is then
clicked by pre-computed extents)."""
import os
import sys
import time
from collections import deque

import pyatspi

TARGET = sys.argv[1] if len(sys.argv) > 1 else "fixtures/linux-cachyos-court2"


def role_name(role):
    if role is None:
        return "?"
    try:
        return str(int(role))
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
        if "cachyos" in app_name or "kernel" in app_name:
            return app
    return None


def wait_app():
    app = None
    deadline = time.time() + 150.0
    while app is None and time.time() < deadline:
        app = find_app()
        if app is None:
            time.sleep(0.5)
    return app


def extents(node):
    try:
        comp = node.get_component()
        if comp is None:
            return None
        return comp.get_extents(0)
    except Exception:
        return None


def click(rect):
    import ctypes
    import ctypes.util
    x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
    xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
    x11.XOpenDisplay.restype = ctypes.c_void_p
    dpy = x11.XOpenDisplay(b":99")
    if not dpy:
        return False
    cx = int(rect.x) + int(rect.width) // 2
    cy = int(rect.y) + int(rect.height) // 2
    xtst.XTestFakeMotionEvent(ctypes.c_void_p(dpy), -1, ctypes.c_int(cx), ctypes.c_int(cy), 0)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 1, 0)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 0, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    return True


def bfs(root):
    out = []
    queue = deque([root])
    while queue:
        node = queue.popleft()
        out.append(node)
        try:
            queue.extend(node[i] for i in range(node.childCount))
        except Exception:
            pass
    return out


def main():
    app = wait_app()
    if app is None:
        print("no app")
        return 1
    nodes = bfs(app)
    # the checkbox whose accessible name is the target identity + the
    # Execute button — BOTH captured BEFORE any click (the accesskit
    # bridge rejects the state-change update a toggle emits, so the tree
    # dies after the first rebuild; the positions do not move on a toggle)
    box = None
    button = None
    for n in nodes:
        try:
            name = (n.name or "").strip()
            txt = node_text(n).strip()
        except Exception:
            continue
        if box is None and name == TARGET:
            box = n
        if button is None and txt == "Execute":
            button = n
    if box is None:
        print("checkbox not found")
        return 2
    if button is None:
        print("execute button not found")
        return 4
    box_rect = extents(box)
    btn_rect = extents(button)
    if box_rect is None:
        print("no checkbox extents")
        return 3
    if btn_rect is None:
        print("no execute extents")
        return 5
    click(box_rect)
    time.sleep(2.5)
    click(btn_rect)
    print("execute clicked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
