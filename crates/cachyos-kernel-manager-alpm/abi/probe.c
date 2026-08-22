/*
 * abi/probe.c — the libalpm ABI court probe.
 *
 * Every ABI fact the hand-written Rust FFI (src/ffi.rs) assumes is checked
 * HERE against the ACTUAL installed headers:
 *
 *   1. compile-time _Static_asserts for layout facts,
 *   2. function-pointer assignments for every extern "C" declaration — a
 *      mismatch in return type, argument type, or calling convention is a
 *      compile error (-Werror in build.rs),
 *   3. a runtime print of the same constants, captured as court evidence
 *      (court alpm-ffi/abi-surface: oracle = this output, candidate = the
 *      Rust-side layout constants).
 *
 * The two historical bugs (list-layout OOM, installed_db SIGSEGV) were both
 * hand-reconstructed ABI facts; this probe makes every such fact machine-
 * verified before the FFI can even compile.
 *
 * Build: cc -Werror -I<pkg-config --cflags libalpm> probe.c -o probe
 */

#include <alpm.h>
#include <alpm_list.h>
#include <stddef.h>
#include <stdio.h>

/* ------------------------------------------------------------------ */
/* layout facts (alpm_list_t is the only non-opaque struct we touch)   */
/* ------------------------------------------------------------------ */

_Static_assert(sizeof(alpm_list_t) == 3 * sizeof(void *),
    "alpm_list_t is not three pointers (data/prev/next); update ffi.rs RawList");
_Static_assert(offsetof(alpm_list_t, data) == 0,
    "alpm_list_t.data is not at offset 0; update ffi.rs RawList");
_Static_assert(offsetof(alpm_list_t, prev) == sizeof(void *),
    "alpm_list_t.prev is not at offset sizeof(void*); update ffi.rs RawList");
_Static_assert(offsetof(alpm_list_t, next) == 2 * sizeof(void *),
    "alpm_list_t.next is not at offset 2*sizeof(void*); update ffi.rs RawList");

/* the Rust side declares the enum args as c_int; the enums must be ints */
_Static_assert(sizeof(alpm_errno_t) == sizeof(int),
    "alpm_errno_t is not int-sized; update ffi.rs (c_int)");
_Static_assert(sizeof(alpm_siglevel_t) == sizeof(int),
    "alpm_siglevel_t is not int-sized; update ffi.rs (c_int)");
_Static_assert(ALPM_SIG_USE_DEFAULT == (1 << 30),
    "ALPM_SIG_USE_DEFAULT changed; update ffi.rs ALPM_SIG_USE_DEFAULT");

/* ------------------------------------------------------------------ */
/* calling signatures — exactly what ffi.rs extern \"C\" declares.      */
/* A mismatch fails the build (-Werror).                               */
/* ------------------------------------------------------------------ */

static alpm_handle_t *(*const chk_alpm_initialize)(const char *, const char *, alpm_errno_t *) =
    alpm_initialize;
static int (*const chk_alpm_release)(alpm_handle_t *) = alpm_release;
static alpm_errno_t (*const chk_alpm_errno)(alpm_handle_t *) = alpm_errno;
static const char *(*const chk_alpm_strerror)(alpm_errno_t) = alpm_strerror;
static alpm_db_t *(*const chk_alpm_register_syncdb)(alpm_handle_t *, const char *, int) =
    alpm_register_syncdb;
static alpm_list_t *(*const chk_alpm_get_syncdbs)(alpm_handle_t *) = alpm_get_syncdbs;
static alpm_db_t *(*const chk_alpm_get_localdb)(alpm_handle_t *) = alpm_get_localdb;
static const char *(*const chk_alpm_db_get_name)(const alpm_db_t *) = alpm_db_get_name;
static alpm_pkg_t *(*const chk_alpm_db_get_pkg)(alpm_db_t *, const char *) = alpm_db_get_pkg;
static alpm_list_t *(*const chk_alpm_db_get_pkgcache)(alpm_db_t *) = alpm_db_get_pkgcache;
static const char *(*const chk_alpm_pkg_get_name)(alpm_pkg_t *) = alpm_pkg_get_name;
static const char *(*const chk_alpm_pkg_get_version)(alpm_pkg_t *) = alpm_pkg_get_version;
static const char *(*const chk_alpm_pkg_get_installed_db)(alpm_pkg_t *) =
    alpm_pkg_get_installed_db;
static int (*const chk_alpm_pkg_vercmp)(const char *, const char *) = alpm_pkg_vercmp;

int main(void) {
    printf("schema=cachyos-km-libalpm-abi-v1\n");
    printf("sizeof(void*)=%zu\n", sizeof(void *));
    printf("sizeof(alpm_list_t)=%zu\n", sizeof(alpm_list_t));
    printf("offsetof(alpm_list_t,data)=%zu\n", offsetof(alpm_list_t, data));
    printf("offsetof(alpm_list_t,prev)=%zu\n", offsetof(alpm_list_t, prev));
    printf("offsetof(alpm_list_t,next)=%zu\n", offsetof(alpm_list_t, next));
    printf("sizeof(alpm_errno_t)=%zu\n", sizeof(alpm_errno_t));
    printf("sizeof(alpm_siglevel_t)=%zu\n", sizeof(alpm_siglevel_t));
    printf("ALPM_SIG_USE_DEFAULT=%d\n", ALPM_SIG_USE_DEFAULT);
    printf("ALPM_PKG_VERCMP_EXPECTED_SIGNATURE=ok\n");
    return 0;
}
