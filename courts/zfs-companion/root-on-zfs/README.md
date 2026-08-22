# zfs-companion/root-on-zfs

Phase 5 transaction court on fixture `zfs-root`.

The oracle side drives the REAL GUI (AT-SPI checkbox toggle + Execute click)
under strace; the candidate side runs the plan tool against the same state.
The comparator compares the exec chains witness-by-witness
(`oracle/oracle-transaction.json` vs `candidate/candidate-transaction.json`).

Select: `fixtures/linux-cachyos-court2`

Expected pacman argv(s): 'pacman' '-S' '--needed' 'linux-cachyos-court2-zfs' 'linux-cachyos-court2' 'linux-cachyos-court2-headers'

Run: `cargo xtask court run zfs-companion/root-on-zfs --vm`
