# ui/close-during-transaction

Phase 12 hostile-review gap-010 court — the race-hunt the audit mandated
("witnessing the oracle's actual crash behavior is race-hunting,
explicitly Phase 12's mandate"). The oracle's closeEvent
(`km-window.cpp:327-338`) releases the alpm handle and lets the app exit
while the worker QThread is still blocked in the transaction; Qt aborts
with `QThread: Destroyed while thread is still running` (SIGABRT). The
candidate's transaction task is a runtime-owned detached thread and the
close exits the event loop — no abort (D-008 INTENTIONAL_CORRECTION).

The court:

1. stages the release binary into the 9p share;
2. boots a fresh overlay of the `close-transaction` fixture (the
   gui-integration X11/AT-SPI stack + a `/usr/local/bin/pacman` wrapper
   that sleeps 15s so the transaction terminal deterministically stays
   in-flight) per side;
3. **oracle side**: drives the frozen Qt GUI — toggle a kernel row +
   Execute — waits for the in-flight terminal, closes the MAIN window via
   WM_DELETE_WINDOW, records the exit outcome;
4. **candidate side**: drives the release Slint binary the same way
   (pre-computed extents — the accesskit bridge rejects the toggle's
   state-change update, so both click positions are captured before the
   first click);
5. compares the machine residuals byte-for-byte (the close corrupts
   nothing on either side) and validates the documented exit-outcome
   expectations (oracle crash, candidate clean).

## The witness (2026-08-23)

- oracle: `exit_outcome=crash rc=134` (SIGABRT, core dumped)
- candidate: `exit_outcome=clean rc=0`

## Run

```sh
cargo build --release --features gui-alpm
cargo xtask vm bake close-transaction
cargo xtask court run ui/close-during-transaction --vm
```

## Falsifiers

Any machine-residual difference, the oracle NOT aborting, the candidate
NOT exiting cleanly, or either side failing to start the transaction.
