# single-instance/stale-lock

Non-VM differential court for the candidate's single-instance lock decision
(`crates/cachyos-kernel-manager-platform/src/single_instance.rs`, rendered
by the `cachyos-kernel-manager-single-instance` bin) vs an independent
re-declaration of the frozen oracle's `IsInstanceAlreadyRunning`
(`tools/singleinstance-oracle-ref`), over the shared corpus
(`cachyos-km-single-instance-corpus-v1`).

The court pins the oracle's `QSharedMemory("CachyOS-KM-lock")` semantics
(`main.cpp:45-56`) for every lock-state scenario:

- **fresh** — `create` succeeds → proceed;
- **held-by-live-process** — `create` fails, attach+detach recover nothing,
  the retry fails → already-running;
- **stale-after-crash** — the crashed holder released the segment (IPC_RMID
  + last-detach destruction), the attach/detach retry clears it, the retry
  succeeds → proceed (the stale-lock RECOVERY gap-001 pins);
- **vanished**/**detach-failure** variants — the retry-only paths.

The candidate's real lock is a `flock(2)`-exclusive file named
`CachyOS-KM-lock` (same name identity, same crash-release property);
the root binary acquires it before any UI init and exits `-1` when held
(`main.cpp:113`).

Status: defined. Run:

```
tools/run-singleinstance-corpus.sh
cargo xtask court run single-instance/stale-lock
```

Falsifier: any byte difference in any decision over any corpus file.
