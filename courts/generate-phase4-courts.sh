#!/usr/bin/env bash
#
# generate-phase4-courts.sh — generate the Phase 4 court case directories:
# adversarial kernel discovery, epoch/version semantics, companion
# resolution, and the pacman-config registration courts.
#
# Regenerate after editing: bash courts/generate-phase4-courts.sh
#
set -euo pipefail
cd "$(dirname "$0")"

# case-id|domain|description|fixture
CASES=(
  "adversarial-names|kernel-discovery|headers-without-kernel, kernel-without-headers, linux-api-headers skip, non-kernel linux-ish packages|adversarial-names"
  "epoch-versions|kernel-discovery|epoch and unusual Arch version syntax in display + upgrade/downgrade markers|epoch-versions"
  "companion-resolution|kernel-discovery|zfs/nvidia/nvidia-open companion presence per kernel (source-anchored model)|companion-resolution"
  "testing-and-disabled|pacman-config|[testing] skipped, [core-testing] registered, commented repos not registered|testing-and-disabled"
  "case-sensitivity|pacman-config|[Fixtures] section lowercased by mINI and discovers fixtures.db (real pacman would not)|case-sensitivity"
  "duplicated-sections|pacman-config|duplicated [fixtures] section merges into one registration (real pacman errors)|duplicated-sections"
  "malformed|pacman-config|malformed pacman.conf: [a=b key, unclosed [broken, stray text, numeric auto-sections|malformed"
  "missing-conf|pacman-config|/etc/pacman.conf absent: zero registrations, empty discovery, No kernels found dialog|missing-conf"
)

for entry in "${CASES[@]}"; do
  id="${entry%%|*}"; rest="${entry#*|}"
  domain="${rest%%|*}"; rest="${rest#*|}"
  desc="${rest%%|*}"; fixture="${rest#*|}"
  dir="$domain/$id"
  mkdir -p "$dir"

  cat > "$dir/claim.toml" <<EOF
# CLAIM — $domain/$id
claim = "Given the '$fixture' fixture, the candidate's kernel discovery produces the same rows (raw name, version text incl. ∨/∧ markers, category, checked state, order) as the oracle's GUI tree, observed through AT-SPI. $desc."

model = "Oracle: real cachyos-kernel-manager v1.19.0 GUI (libalpm discovery per kernel.cpp:179-286, mINI pacman.conf registration per alpm_utils.cpp:32-47 — sections lowercased, testing/options skipped, Include NOT followed, duplicates merged, display per kernel.cpp:56-79 + km-window.cpp:89-106). Candidate: cachyos-kernel-manager-inspect over the same libalpm databases + the same registration/display rules."

assumptions = [
  "Both runs start from fresh copy-on-write overlays of the SAME baked fixture image (identical machine state).",
  "The oracle runs under Xvfb with AT-SPI enabled; the tree is read from the accessibility tree, never screenshots.",
  "The '$fixture' fixture is baked offline in a chroot (loop-free), never by booting and mutating a running system.",
]

observables = [
  "Ordered kernel rows: raw name, version column text, category text, checkbox state",
  "Critical dialog presence/text (No kernels found!) when discovery is empty",
  "Machine residual identity between the two runs (fixture-integrity)",
  "Candidate companion candidates (zfs/nvidia/nvidia-open) against the source-anchored model when comparator.toml provides one",
]

witness = "cargo xtask court run $domain/$id --vm against fixture '$fixture'."

independence = "The oracle is the real upstream binary executing against real libalpm in a disposable VM; the candidate is a separate Rust implementation observing the identical fixture. Neither generates the other's output."

falsifier = "Any difference in row set, row order, version text, category, checked state, the empty-discovery dialog condition, or (where modeled) companion candidates."

[[evidence]]
artifact = "oracle/oracle-state.json"
sha256 = "pending"

[[evidence]]
artifact = "candidate/candidate-state.json"
sha256 = "pending"
EOF

  cat > "$dir/assumptions.toml" <<EOF
assumptions = [
  "The oracle and the candidate register sync databases from /etc/pacman.conf with the SAME rule (mINI sections, testing/options skipped, in file order).",
  "Sync db files are read from /var/lib/pacman/sync/; no refresh/network happens during discovery.",
  "libalpm is the authoritative version comparator on both sides (alpm_pkg_vercmp).",
]

[[normalizers]]
name = "a11y-rows"
version = "1.1.0"
source_hash = "pending"
input_domain = "oracle-state.json (schema cachyos-km-oracle-a11y-v1)"
transformation = "extract ordered kernel rows + dialogs from the flat Qt TABLE_CELL tree (numeric AT-SPI roles)"
justification = "Qt exposes the kernel QTreeWidget as a flat cell list; role ids are the canonical raw evidence"
falsifier = "a row set/order/version/category/checked difference between oracle and candidate"
EOF

  cat > "$dir/comparator.toml" <<EOF
version = "1.0.0"
fixture = "$fixture"

ignore = ["oracle.stdout", "oracle.stderr", "candidate.stderr", "oracle-trace.log"]
byte_exact = []
json_semantic = ["oracle-state.json", "candidate-state.json", "residual.json"]
volatile_prefixes = ["/tmp/", "/proc/", "/run/user/", "/var/log/"]
EOF

  cat > "$dir/README.md" <<EOF
# $domain/$id

$desc.

Status: defined. Execution: \`cargo xtask court run $domain/$id --vm\`
(requires the baked fixture: \`cargo xtask vm bake $fixture\`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
EOF

  echo "generated $dir"
done
echo "done: $(ls -d */ | wc -l) domains, 8 cases"
