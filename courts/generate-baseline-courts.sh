#!/usr/bin/env bash
#
# generate-baseline-courts.sh — generate the Phase 2 baseline court case
# directories (courts/kernel-discovery/<fixture>/) for the fixture matrix.
#
# Regenerate after editing: bash courts/generate-baseline-courts.sh
#
set -euo pipefail
cd "$(dirname "$0")"

# fixture-name|short description (also used in claim/assumption text)
CASES=(
  "minimal|base state: linux-cachyos installed from cachyos, all sync dbs as built"
  "several-kernels|real kernels from core/extra/cachyos installed (linux, linux-lts, linux-zen, linux-cachyos-lts, linux-cachyos-rt-bore)"
  "upgrade-available|fake kernel: local 9.8.8 < sync 9.9.9 (∧ marker, update flag)"
  "downgrade-visible|fake kernel: local 9.9.9 > sync 9.8.8 (∨ marker, no update flag)"
  "custom-repo|fake kernel only in the [fixtures] repo, not installed"
  "cross-repo-installed|fake kernel installed from [fixtures], also present in [other] (mutable row)"
  "duplicate-across-repos|same fake kernel name in [fixtures] and [other] with different versions"
  "stale-db|cachyos sync db removed; cachyos/linux-cachyos vanishes from discovery"
  "empty-all-dbs|no sync databases at all; oracle must show the No kernels found dialog"
  "empty-sync-db|[emptyrepo] sync db present with zero packages"
)

for entry in "${CASES[@]}"; do
  name="${entry%%|*}"
  desc="${entry#*|}"
  dir="kernel-discovery/$name"
  mkdir -p "$dir"

  cat > "$dir/claim.toml" <<EOF
# CLAIM — kernel-discovery/$name
claim = "Given the '$name' fixture, the candidate's kernel discovery produces the same rows (raw name, version text incl. ∨/∧ markers, category, checked state, order) as the oracle's GUI tree, observed through AT-SPI."

model = "Oracle: real cachyos-kernel-manager v1.19.0 GUI (libalpm discovery per kernel.cpp:179-286, mINI pacman.conf registration per alpm_utils.cpp:32-47, display per kernel.cpp:56-79 + km-window.cpp:89-106). Candidate: cachyos-kernel-manager-inspect over the same libalpm databases + the same registration/display rules."

assumptions = [
  "Both runs start from fresh copy-on-write overlays of the SAME baked fixture image (identical machine state).",
  "The oracle runs under Xvfb with AT-SPI enabled; the tree is read from the accessibility tree, never screenshots.",
  "The '$name' fixture is baked offline (losetup + chroot), never by booting and mutating a running system.",
]

observables = [
  "Ordered kernel rows: raw name, version column text, category text, checkbox state",
  "Critical dialog presence/text (No kernels found!) when discovery is empty",
  "Machine residual identity between the two runs (fixture-integrity)",
]

witness = "cargo xtask court run kernel-discovery/$name --vm against fixture '$name'."

independence = "The oracle is the real upstream binary executing against real libalpm in a disposable VM; the candidate is a separate Rust implementation observing the identical fixture. Neither generates the other's output."

falsifier = "Any difference in row set, row order, version text, category, checked state, or the empty-discovery dialog condition."

[[evidence]]
artifact = "oracle/oracle-state.json"
sha256 = "pending"

[[evidence]]
artifact = "candidate/candidate-state.json"
sha256 = "pending"
EOF

  cat > "$dir/assumptions.toml" <<EOF
assumptions = [
  "Fixture: $desc",
  "Disposable court VM with the fixture marker (/etc/cachyos-km/fixture.marker); destructive courts fail closed without it.",
  "Sync databases are NOT refreshed during the run (no -Sy).",
  "AUR support disabled in the oracle build (default).",
]

[[normalizers]]
name = "at-spi-tree-extraction"
version = "1.0.0"
source_hash = "pending"
input_domain = "oracle-state.json (full at-spi tree)"
transformation = "extract ordered kernel rows (raw/version/category/checked) + dialogs; drop PIDs, dbus names, coordinates"
justification = "The at-spi tree contains presentation metadata not part of the behavioral contract; row identity/order/text IS the contract."
falsifier = "A row that the extractor drops or reorders is a real residual (the raw tree remains in evidence)."
EOF

  cat > "$dir/comparator.toml" <<EOF
version = "1.0.0"
fixture = "$name"

# The VM comparison path uses the normalizers; these lists document the
# raw-evidence comparison policy.
ignore = ["oracle.stdout", "oracle.stderr", "candidate.stderr", "oracle-trace.log"]
byte_exact = []
json_semantic = ["oracle-state.json", "candidate-state.json", "residual.json"]
volatile_prefixes = ["/tmp/", "/proc/", "/run/user/", "/var/log/"]
EOF

  cat > "$dir/README.md" <<EOF
# kernel-discovery/$name

$desc

Status: defined. Execution: \`cargo xtask court run kernel-discovery/$name --vm\`
(requires the baked base image and this fixture: \`cargo xtask vm bake $name\`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
EOF

  echo "generated $dir"
done
echo "baseline courts generated"
