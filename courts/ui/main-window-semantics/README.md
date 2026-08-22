# ui/main-window-semantics

Non-VM differential court for the candidate's main-window semantic model
(`crates/cachyos-kernel-manager-ui/src/main_window.rs`, rendered by the
`cachyos-kernel-manager-mainwindow` bin) vs an independent re-declaration
of the frozen oracle's `km-window.cpp` / `kernel.cpp` semantics
(`tools/mainwindow-oracle-ref`), over the shared corpus
(`cachyos-km-main-window-corpus-v1`).

The court pins, for every corpus scenario (minimal, installed-immutable,
cross-repo-installed, update/downgrade visibility, AUR rows, toggles,
transaction-running, double-toggle):

- the tree rows (`init_kernels_tree_widget`, `km-window.cpp:89-106`) with
  the installed-db provenance rule;
- the version text (`Kernel::version`, `kernel.cpp:56-79`): the AUR
  short-circuit, the ∨/∧ vercmp markers, the update flag;
- the OK-button enablement + the change list (`build_change_list`,
  `km-window.cpp:304-325`, worker enablement at 125/174);
- the sched-ext button visibility (`km-window.cpp:185-188`);
- the version-column sort keys (`operator<`, `km-window.cpp:391-412`);
- the space-toggle guard (`check_uncheck_item`, `km-window.cpp:285-293`).

The exact alpm vercmp is courted by the version-state/epoch courts; this
court fixes the comparator and pins the **row assembly** — the semantics
the Iced tree widget will be built on (Phase 8).

Status: defined. Run:

```
tools/run-mainwindow-corpus.sh
cargo xtask court run ui/main-window-semantics
```

Falsifier: any byte difference in any field of any row, or in the derived
enablement/list/sort/toggle values, over any corpus file.
