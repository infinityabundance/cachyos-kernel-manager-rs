#!/usr/bin/env python3
"""xclose.py — send WM_DELETE_WINDOW to the window whose WM_NAME contains
the given substring (the Qt app's closeEvent path — no WM in the court
VMs, so the close is synthesized via a ClientMessage, exactly what a WM
would send)."""

import ctypes
import ctypes.util
import struct
import sys

x11 = ctypes.CDLL(ctypes.util.find_library("X11"))

DisplayP = ctypes.c_void_p
Window = ctypes.c_ulong
Atom = ctypes.c_ulong

x11.XOpenDisplay.restype = DisplayP
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XDefaultRootWindow.restype = Window
x11.XDefaultRootWindow.argtypes = [DisplayP]
x11.XInternAtom.restype = Atom
x11.XInternAtom.argtypes = [DisplayP, ctypes.c_char_p, ctypes.c_int]
x11.XQueryTree.restype = ctypes.c_int
x11.XQueryTree.argtypes = [DisplayP, Window, ctypes.POINTER(Window),
                           ctypes.POINTER(Window), ctypes.POINTER(ctypes.POINTER(Window)),
                           ctypes.POINTER(ctypes.c_uint)]
x11.XFree.restype = ctypes.c_int
x11.XFree.argtypes = [ctypes.c_void_p]
x11.XGetWMName.restype = ctypes.c_int
x11.XGetWMName.argtypes = [DisplayP, Window, ctypes.c_void_p]
x11.XSendEvent.restype = ctypes.c_int
x11.XSendEvent.argtypes = [DisplayP, Window, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
x11.XFlush.argtypes = [DisplayP]
x11.XFlush.restype = ctypes.c_int

needle = sys.argv[1] if len(sys.argv) > 1 else "CachyOS"

dpy = x11.XOpenDisplay(b":99")
if not dpy:
    print("no display")
    sys.exit(1)
root = x11.XDefaultRootWindow(dpy)


def children_of(win):
    root_r = Window()
    parent_r = Window()
    children = ctypes.POINTER(Window)()
    n = ctypes.c_uint()
    rc = x11.XQueryTree(dpy, win, ctypes.byref(root_r), ctypes.byref(parent_r),
                        ctypes.byref(children), ctypes.byref(n))
    out = []
    if rc and n.value:
        out = [children[i] for i in range(n.value)]
        x11.XFree(ctypes.cast(children, ctypes.c_void_p))
    return out


target = None


def walk(win, depth=0):
    global target
    if depth > 8:
        return
    class TP(ctypes.Structure):
        _fields_ = [("value", ctypes.c_void_p), ("encoding", Atom),
                    ("format", ctypes.c_int), ("nitems", ctypes.c_ulong)]
    tp = TP()
    name = ""
    if x11.XGetWMName(dpy, win, ctypes.byref(tp)) and tp.value:
        name = ctypes.string_at(tp.value).decode("utf-8", "replace")
    if needle.lower() == name.lower():
        target = win  # EXACT match wins ("CachyOS Kernel Manager" is a
        return          # PREFIX of the Configure window's title)
    for c in children_of(win):
        walk(c, depth + 1)


def walk_any(win, depth=0):
    global target
    if target is not None or depth > 8:
        return
    class TP(ctypes.Structure):
        _fields_ = [("value", ctypes.c_void_p), ("encoding", Atom),
                    ("format", ctypes.c_int), ("nitems", ctypes.c_ulong)]
    tp = TP()
    name = ""
    if x11.XGetWMName(dpy, win, ctypes.byref(tp)) and tp.value:
        name = ctypes.string_at(tp.value).decode("utf-8", "replace")
        if needle.lower() in name.lower():
            target = win
            return
    for c in children_of(win):
        walk_any(c, depth + 1)


walk(root)
if target is None:
    walk_any(root)
if target is None:
    print("window not found")
    sys.exit(2)

wm_protocols = x11.XInternAtom(dpy, b"WM_PROTOCOLS", 0)
wm_delete = x11.XInternAtom(dpy, b"WM_DELETE_WINDOW", 0)

# XClientMessageEvent (64-bit Xlib): type@0 serial@8 send_event@16
# display@24 window@32 message_type@40 format@48 data.l@56 (5 longs)
buf = (ctypes.c_byte * 192)()  # sizeof(XEvent); zeroed beyond the fields
struct.pack_into("@i", buf, 0, 33)            # ClientMessage
struct.pack_into("@i", buf, 16, 1)            # send_event = True
struct.pack_into("@Q", buf, 24, int(dpy))     # display
struct.pack_into("@Q", buf, 32, int(target))  # window
struct.pack_into("@Q", buf, 40, int(wm_protocols))  # message_type
struct.pack_into("@i", buf, 48, 32)           # format
struct.pack_into("@q", buf, 56, int(wm_delete))     # data.l[0]
struct.pack_into("@q", buf, 64, 0)            # data.l[1]

evt = ctypes.cast(buf, ctypes.c_void_p)
x11.XSendEvent(dpy, target, 0, 0, evt)
x11.XFlush(dpy)
print(f"WM_DELETE_WINDOW sent to {target:#x} ({needle})")
