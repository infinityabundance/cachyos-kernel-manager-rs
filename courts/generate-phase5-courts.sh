#!/usr/bin/env bash
#
# generate-phase5-courts.sh — scaffold the Phase 5 court case directories
# (nvidia/zfs companion decision matrix, kernel removal, update quirk,
# terminal-helper matrix). Each court gets claim.toml, assumptions.toml,
# comparator.toml and README.md; the fixtures must be baked first
# (`cargo xtask vm bake <fixture>`).
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COURTS="$ROOT/courts"

mkcase() { # mkcase <domain> <case> <fixture> <select> <expect-argv...>
    local domain="$1" case="$2" fixture="$3" select="$4"
    shift 4
    local dir="$COURTS/$domain/$case"
    mkdir -p "$dir"
    # argv expectation joined for the claim
    local expect_argv=""
    for a in "$@"; do
        expect_argv="$expect_argv '$a'"
    done

    cat > "$dir/comparator.toml" <<EOF
version = "1.2.0"
fixture = "$fixture"

ignore = ["oracle.stdout", "oracle.stderr", "candidate.stderr"]
byte_exact = []
json_semantic = ["oracle-state.json", "candidate-state.json", "residual.json", "candidate-residual.json"]
volatile_prefixes = ["/tmp/", "/proc/", "/run/user/", "/var/log/", "/dev/", "/sys/", "/sys/fs/", "/sys/kernel/"]

[transaction]
select = ["$select"]
EOF

    cat > "$dir/assumptions.toml" <<EOF
assumptions = [
  "Both runs start from fresh copy-on-write overlays of the SAME baked fixture image (identical machine state).",
  "The oracle runs under Xvfb with AT-SPI enabled; the transaction is driven through the accessibility tree (checkbox toggle + Execute click), never by coordinates.",
  "The exec-chain witness is the strace execve log of the real oracle process tree; the candidate's chain is modeled by the plan tool against the same libalpm state.",
  "The chwd/findmnt wrappers are the narrowest verifiable simulation boundary for the hardware probes (the real binaries would answer with VM-dependent values).",
  "The polkit test rule in the base image authorizes the oracle's action without a password (TEST-ONLY, never shipped).",
  "A real xterm is installed so the terminal-helper chain actually reaches the pacman execve; pacman never completes a transaction (killed at the confirmation prompt).",
]

normalizers = [
  { name = "strace-execve", version = "1.0.0", source_hash = "vm/in-vm/extract-transaction.py", input_domain = "oracle-trace.log execve lines", transformation = "extract probe/exec/terminal argv chains in order", justification = "the raw trace is preserved; the chains are the comparable observables", falsifier = "a chain element that differs between oracle and candidate" },
  { name = "machine-residual", version = "1.0.0", source_hash = "vm/in-vm/residual.sh", input_domain = "machine state before each run", transformation = "hash the installed package list + db hashes", justification = "fixture-integrity check", falsifier = "any drift between the two runs" },
]
EOF

    cat > "$dir/claim.toml" <<EOF
claim = "Given the '$fixture' fixture, the oracle's real GUI transaction (toggle '$select' + Execute) produces exactly the exec chain the candidate plan tool models: the same probe commands in the same order and the same pacman argv(s):$expect_argv (plus the same terminal-helper invocation)."

model = "Oracle: real cachyos-kernel-manager v1.19.0 GUI. Static-init probes (kernel.cpp:41-52): findmnt -ln -o FSTYPE /, chwd --list-installed -d pipeline (evaluated TWICE). Install phase per selected kernel (kernel.cpp:89-135): pacman -Qqs '^linux-cachyos.*-nvidia$', then '^linux-cachyos.*-nvidia-open$', then the companion decision matrix (zfs-root first, then chwd/modules/dkms nvidia logic), then kernel + headers. Removal phase (kernel.cpp:137-163): kernel then installed companions. commit_transaction (kernel.cpp:288-304): pacman -S --needed <install> then pacman -Rsn <remove>, each through runCmdTerminal (utils.cpp:122-135: '; read -p ...' suffix + -s pkexec rootshell.sh). Candidate: the plan tool over the same libalpm state + the same modeled chains."

assumptions = [
  "The fixture controls the hardware probes (chwd/findmnt wrappers) and the package state; both sides see byte-identical inputs.",
  "The AT-SPI driver toggles each target row's checkbox from its default state (checked when installed+immutable, unchecked otherwise), exactly what 'select' means.",
]

observables = [
  "Ordered probe exec chains (findmnt, chwd pipelines, pacman -Qqs)",
  "Ordered transaction pacman argv (pacman -S --needed ... / pacman -Rsn ...)",
  "The terminal-helper invocation argv",
  "Kernel rows of the SAME run (discovery parity still holds)",
  "Machine residual identity between the two runs (fixture-integrity)",
]

witness = "cargo xtask court run $domain/$case --vm against fixture '$fixture'."

independence = "The oracle is the real upstream binary driven through its real GUI in a disposable VM; the candidate is a separate Rust implementation modeling the same state. Neither generates the other's output; the exec chains are witnessed from the operating system (strace), not from the application."

falsifier = "Any difference in probe order, probe argv, transaction pacman argv (order-sensitive), terminal-helper argv, kernel rows, or machine residual drift."

[[evidence]]
artifact = "oracle/oracle-transaction.json"
sha256 = "pending"

[[evidence]]
artifact = "candidate/candidate-transaction.json"
sha256 = "pending"

[[evidence]]
artifact = "oracle/oracle-trace.log"
sha256 = "pending"
EOF

    cat > "$dir/README.md" <<EOF
# $domain/$case

Phase 5 transaction court on fixture \`$fixture\`.

The oracle side drives the REAL GUI (AT-SPI checkbox toggle + Execute click)
under strace; the candidate side runs the plan tool against the same state.
The comparator compares the exec chains witness-by-witness
(\`oracle/oracle-transaction.json\` vs \`candidate/candidate-transaction.json\`).

Select: \`$select\`

Expected pacman argv(s):$expect_argv

Run: \`cargo xtask court run $domain/$case --vm\`
EOF
    echo "scaffolded $domain/$case"
}

mkcase nvidia-companion dkms-profile nvidia-dkms-profile fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2-nvidia linux-cachyos-court2 linux-cachyos-court2-headers
mkcase nvidia-companion open-profile nvidia-open-profile fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2-nvidia-open linux-cachyos-court2 linux-cachyos-court2-headers
mkcase nvidia-companion dkms-installed nvidia-dkms-installed fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2 linux-cachyos-court2-headers
mkcase nvidia-companion modules-installed nvidia-modules-installed fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2-nvidia linux-cachyos-court2 linux-cachyos-court2-headers
mkcase zfs-companion root-on-zfs zfs-root fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2-zfs linux-cachyos-court2 linux-cachyos-court2-headers
mkcase kernel-removal plan removal-plan fixtures/linux-cachyos-court2 \
    pacman -Rsn linux-cachyos-court2 linux-cachyos-court2-headers linux-cachyos-court2-zfs linux-cachyos-court2-nvidia
mkcase kernel-removal update-available-execute update-available-execute fixtures/linux-cachyos-court2 \
    pacman -S --needed linux-cachyos-court2 linux-cachyos-court2-headers \
    pacman -Rsn linux-cachyos-court2 linux-cachyos-court2-headers

echo "phase5 courts scaffolded"
