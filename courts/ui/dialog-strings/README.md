# ui/dialog-strings

Non-VM differential court for the candidate's user-visible string table:
every string the Iced UI will render (window titles, tree columns, buttons,
variant labels, combo options, progress labels, dialogs, stdout/stderr
lines) must be **byte-identical** to the frozen oracle's strings.

- **Oracle side**: `tools/strings-oracle-ref` re-declares the strings
  directly from the frozen source (`oracle/upstream/src` @ `6b4a373e`; the
  sched-ext strings from the pre-extraction scx-manager @ `f3eeaf6` in
  `oracle/scx-authority/`), with the file:line reference of every string.
- **Candidate side**: `crates/cachyos-kernel-manager-ui/src/strings.rs`
  (rendered by the `cachyos-kernel-manager-strings` bin) — the table the
  Iced UI is built from.

Both hand-writings were produced **independently** from the same authority;
the court catches any drift between them. The descriptor is
`cachyos-km-strings-v1`: a JSON array of `(id, source, text)` rows in a
fixed order.

The string table is a **fixed contract** — there is no corpus. The fixture
directory exists only for layout symmetry with the corpus-driven courts.

Status: defined. Run:

```
tools/run-strings-corpus.sh
cargo xtask court run ui/dialog-strings
```

Falsifier: any byte difference in any string, id, or source reference.
