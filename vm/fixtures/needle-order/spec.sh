#!/usr/bin/env bash
#
# needle-order — gap-002 adversarial needle db: MANY needle-matching
# packages with shuffled names + adversarial interleavings, to stress the
# alpm_db_search ordering (the oracle, kernel.cpp:184-197) against the
# candidate's pkgcache-order discovery.
#
# The hash-iteration order of libalpm's pkgcache is what BOTH sides observe
# (alpm_db_search iterates the same hash); this fixture makes that order
# non-trivial: 40 kernels + headers with name prefixes that interleave in
# the hash, plus api-headers-lookalikes and headers-without-kernel traps
# interleaved at every position.
#
# The court (kernel-discovery/needle-order) witnesses that the row ORDER
# (and the full row set) is identical on the real libalpm.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

ensure_fixtures_repo

# 40 shuffled kernel families: kA0..kA9, kB0..kB9, ... with the headers
# needle matching; headers built BEFORE their kernels so the db cache holds
# the pairs interleaved (headers at even positions in insert order).
for fam in A B C D; do
    for i in 0 1 2 3 4 5 6 7 8 9; do
        name="linux-k${fam}${i}"
        build_fakepkg "${name}-headers" 9.${fam}${i}.1 1
        build_fakepkg "$name" 9.${fam}${i}.1 1
    done
done

# adversarial interleavings
build_fakepkg linux-api-headers 9.0.0 1        # skipped (linux-api-headers)
build_fakepkg linux-api-headers-dev 9.0.0 1    # 'linux-api-headers' substring -> skipped
build_fakepkg linux-lonely-headers 9.0.0 1     # headers WITHOUT a kernel -> skipped
build_fakepkg linux-orphan 9.0.0 1             # kernel WITHOUT headers -> invisible
build_fakepkg linux-cachyos-needle 9.0.0 1
build_fakepkg linux-cachyos-needle-headers 9.0.0 1
build_fakepkg notlinux-headers 9.0.0 1         # needle requires the 'linux' prefix
build_fakepkg linux 9.0.0 1                    # bare 'linux' kernel
build_fakepkg linux-headers 9.0.0 1            # its headers (matches the needle)
build_fakepkg linux-lts 9.0.0 1
build_fakepkg linux-lts-headers 9.0.0 1

repo_add_all
pacman_sync
echo "fixture needle-order: 40 kernel families + api/lonely/orphan/cachyos-needle/bare-linux traps"
