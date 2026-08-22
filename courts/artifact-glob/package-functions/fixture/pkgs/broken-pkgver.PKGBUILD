# functions exist but the PKGBUILD is BROKEN: a top-level `exit` makes the
# probe process exit before the `declare -F;echo "pkgver: ..."` line, so the
# probe output has NO "pkgver: " line -> the oracle error path
# ("broken pkgbuild; pkgver must be present", empty globs).
pkgname=linux-cachyos
exit 1
package_linux-cachyos() {
    true
}
package_linux-cachyos-headers() {
    true
}
