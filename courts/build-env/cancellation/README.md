# build-env/cancellation

Non-VM differential court for the oracle's Configure-window lifecycle
(`on_execute` `m_running` guard + `closeEvent`/`on_cancel`,
`conf-window.cpp:549-550,688-701`): the candidate's `configure_trace` model
(exec crate) vs the source-derived oracle reference, over a frozen 6-case
corpus of action sequences:

| corpus file | exercises |
|---|---|
| `double-execute-then-cancel.json` | the m_running guard: a second Execute while running is a no-op; Cancel closes |
| `cancel-during-build.json` | Cancel while a build runs |
| `close-during-build.json` | the WM close during a build (default closeEvent accepts) |
| `cancel-idle.json` | Cancel with nothing running (close is still accepted) |
| `execute-twice-no-close.json` | two Executes, no close → final m_running=true |
| `empty.json` | no actions → empty trace |

Covers: the `if (m_running) { return; }` guard (a second OK click emits
nothing), the unconditional close (`QWidget::closeEvent` — no confirmation,
no blocking), the terminal nature of a close/cancel (the window — and its
`QProcess m_cmd` member — is destroyed; the destructor terminates the
in-flight child, which IS the oracle's build cancellation), and the
`m_running` transitions.

Witness: `tools/run-cancel-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run build-env/cancellation` byte-compares them.

Status: defined. Run:

```
tools/run-cancel-corpus.sh
cargo xtask court run build-env/cancellation
```

Falsifier: any byte difference in the trace or the final running state on
any corpus case.
