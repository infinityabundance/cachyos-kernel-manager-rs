# fixtures/

Static fixture corpora for courts and tests (directive §41, §21, §23):

- pacman.conf variants (ordinary, testing, custom repos, include, malformed,
  empty, ordering)
- package DB dumps (kernel-discovery courts)
- chwd / findmnt output samples (nvidia/zfs courts)
- PKGBUILD samples (patch-injection, custom-name courts)
- config TOML corpus (current, historical, malformed, unknown fields,
  Unicode custom names)

Fixtures are immutable; courts hash them (`fixture_digest`).
Populated as the corresponding courts are implemented.
