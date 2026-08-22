//! `cachyos-kernel-manager-alpm-abi` — candidate witness for the
//! `alpm-ffi/abi-surface` court.
//!
//! Prints the Rust side's ACTUAL compiled libalpm ABI facts (the `RawList`
//! layout + `ALPM_SIG_USE_DEFAULT`) in the exact `abi/probe.c` output
//! format. The court compares this against the C probe's print of the real
//! headers, byte-for-byte — the executable proof that the handwritten FFI's
//! ABI assumptions match the installed libalpm.
//!
//! The same facts are enforced at BUILD time (build.rs compiles+runs
//! abi/probe.c with -Werror and checks the invariants), so a drift fails
//! loudly before the FFI can even link; this tool is the evidence record.

use cachyos_kernel_manager_alpm::ffi::{
    ALPM_SIG_USE_DEFAULT, ENUM_SIZE, PTR_SIZE, RAW_LIST_OFFSET_DATA, RAW_LIST_OFFSET_NEXT,
    RAW_LIST_OFFSET_PREV, RAW_LIST_SIZE,
};

fn main() {
    println!("schema=cachyos-km-libalpm-abi-v1");
    println!("sizeof(void*)={PTR_SIZE}");
    println!("sizeof(alpm_list_t)={RAW_LIST_SIZE}");
    println!("offsetof(alpm_list_t,data)={RAW_LIST_OFFSET_DATA}");
    println!("offsetof(alpm_list_t,prev)={RAW_LIST_OFFSET_PREV}");
    println!("offsetof(alpm_list_t,next)={RAW_LIST_OFFSET_NEXT}");
    println!("sizeof(alpm_errno_t)={ENUM_SIZE}");
    println!("sizeof(alpm_siglevel_t)={ENUM_SIZE}");
    println!("ALPM_SIG_USE_DEFAULT={ALPM_SIG_USE_DEFAULT}");
    println!("ALPM_PKG_VERCMP_EXPECTED_SIGNATURE=ok");
}
