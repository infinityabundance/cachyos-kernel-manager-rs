#!/usr/bin/env python3
"""a11y-status-stub.py — flip org.a11y.Status.IsEnabled for accesskit_unix.

at-spi2-core 2.60.6 serves org.a11y.Status (IsEnabled) on the org.a11y.Bus
launcher, but leaves IsEnabled FALSE forever — nothing in the modern
at-spi2 stack flips it. accesskit_unix 0.22.1 (slint 1.17.1's a11y stack)
subscribes to IsEnabledChanged and only creates its at-spi Bus (which
registers the app with the registry) on a false->true transition, so the
Slint GUI never appears on the AT-SPI registry in the court VMs (verified
2026-08-23: the app runs and creates its windows but never connects to the
at-spi socket).

This stub drives the LAUNCHER'S OWN readwrite property through
false->true transitions on a schedule: whenever the app's accesskit
subscribes, the next transition completes its enablement dance
(GetAddress -> connect -> register with the registry).

Runtime requirement: PyGObject (gi) + a GLib main loop. Run on the SAME
session bus as the app:

    python3 a11y-status-stub.py &   # after dbus-launch, before the app
"""

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

SERVICE = "org.a11y.Bus"
PATH = "/org/a11y/bus"
IFACE_STATUS = "org.a11y.Status"
IFACE_PROPS = "org.freedesktop.DBus.Properties"

# square-wave period: flip every 4s (false -> true -> false -> true ...).
# The app's accesskit subscribes within ~1-3s of startup; a 4s period means
# the next true transition lands within at most 8s.
PERIOD_SECONDS = 4


def set_enabled(conn: Gio.DBusConnection, value: bool):
    variant = GLib.Variant("(ssv)", (IFACE_STATUS, "IsEnabled", GLib.Variant("b", value)))
    try:
        conn.call_sync(
            SERVICE,
            PATH,
            IFACE_PROPS,
            "Set",
            variant,
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
        print(f"a11y-status-stub: IsEnabled -> {value}", flush=True)
    except Exception as e:
        print(f"a11y-status-stub: set failed: {e}", flush=True)


def main():
    conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    state = {"on": False}

    def flip():
        state["on"] = not state["on"]
        set_enabled(conn, state["on"])
        return True  # keep the timer

    GLib.timeout_add_seconds(PERIOD_SECONDS, flip)
    print("a11y-status-stub: flipping org.a11y.Status.IsEnabled on the session bus", flush=True)
    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
