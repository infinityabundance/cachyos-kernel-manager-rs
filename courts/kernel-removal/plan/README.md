# kernel-removal/plan

Phase 5 transaction court on fixture `removal-plan`.

The oracle side drives the REAL GUI (AT-SPI checkbox toggle + Execute click)
under strace; the candidate side runs the plan tool against the same state.
The comparator compares the exec chains witness-by-witness
(`oracle/oracle-transaction.json` vs `candidate/candidate-transaction.json`).

Select: `fixtures/linux-cachyos-court2`

Expected pacman argv(s): 'pacman' '-Rsn' 'linux-cachyos-court2' 'linux-cachyos-court2-headers' 'linux-cachyos-court2-zfs' 'linux-cachyos-court2-nvidia'

Run: `cargo xtask court run kernel-removal/plan --vm`
