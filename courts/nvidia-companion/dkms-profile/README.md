# nvidia-companion/dkms-profile

Phase 5 transaction court on fixture `nvidia-dkms-profile`.

The oracle side drives the REAL GUI (AT-SPI checkbox toggle + Execute click)
under strace; the candidate side runs the plan tool against the same state.
The comparator compares the exec chains witness-by-witness
(`oracle/oracle-transaction.json` vs `candidate/candidate-transaction.json`).

Select: `fixtures/linux-cachyos-court2`

Expected pacman argv(s): 'pacman' '-S' '--needed' 'linux-cachyos-court2-nvidia' 'linux-cachyos-court2' 'linux-cachyos-court2-headers'

Run: `cargo xtask court run nvidia-companion/dkms-profile --vm`
