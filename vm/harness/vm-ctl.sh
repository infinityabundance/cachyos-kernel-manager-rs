#!/usr/bin/env bash
#
# vm-ctl.sh — host-side QEMU/KVM VM control harness.
#
# Usage:
#   vm-ctl.sh start <overlay.qcow2> [extra-qemu-args...]
#   vm-ctl.sh exec <command...>
#   vm-ctl.sh put <host-file> <vm-absolute-path>
#   vm-ctl.sh stop
#
# The VM boots with:
#   - the given overlay qcow2 (backed by a fixture image)
#   - QEMU direct-kernel boot (vm/images/boot/{vmlinuz,initramfs}) with
#     root=LABEL=cachyoskmroot — no bootloader in the image
#   - SSH on 127.0.0.1:2222 (harness key)
#   - a 9p share at vm/images/share mounted on /mnt/host in the VM
#   - KVM acceleration, host CPU, 8 vCPUs, 8 GiB RAM (default; tunable via
#     KM_VM_MEM — see "OOM protection" below)
#
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
IMAGES="$HERE/../images"
KEY="$IMAGES/harness_key"
SHARE="$IMAGES/share"
PIDFILE="$IMAGES/vm.pid"
SSHPORT="${KM_VM_SSHPORT:-2222}"
SSHOPTS=(-i "$KEY" -p "$SSHPORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=5)

# direct-kernel boot files (copied from the base image at build time)
VMLINUZ="$(ls "$IMAGES"/boot/vmlinuz-* 2>/dev/null | head -1 || true)"
INITRD="$(ls "$IMAGES"/boot/initramfs-linux-cachyos.img 2>/dev/null | head -1 || true)"
KERNEL_CMDLINE="root=LABEL=cachyoskmroot console=ttyS0 rw"

ssh_run() { ssh "${SSHOPTS[@]}" root@127.0.0.1 "$@"; }

# bounded ssh wait (default 360 iterations; the failed-boot court caps it
# via KM_WAIT_SSH_TIMEOUT because a machine whose kernel was removed does
# NOT become usable — the no-ssh outcome IS the expected witness)
WAIT_SSH_TIMEOUT="${KM_WAIT_SSH_TIMEOUT:-360}"

wait_ssh() {
    # Boot time is strongly host-dependent: on an idle host the guest is up
    # in ~10s, but under host swap/I/O pressure it has taken 4+ minutes
    # (observed on this development host). Poll generously and log progress
    # so slow boots are diagnosable instead of silent timeouts.
    local tries=0
    local report=0
    while [ "$tries" -lt "$WAIT_SSH_TIMEOUT" ]; do
        if ssh_run true >/dev/null 2>&1; then return 0; fi
        tries=$((tries + 1))
        if [ $((tries % 30)) -eq 0 ]; then
            report=$((report + 1))
            echo "vm-ctl: waiting for ssh... ${tries}s (host-dependent boot; still waiting)" >&2
        fi
        sleep 1
    done
    echo "vm-ctl: ssh did not become ready in ${WAIT_SSH_TIMEOUT}s" >&2
    return 1
}

cmd_start() {
    local overlay="$1"; shift || true
    [ -f "$overlay" ] || { echo "vm-ctl: overlay missing: $overlay" >&2; return 1; }
    [ -f "$KEY" ] || { echo "vm-ctl: harness key missing: $KEY" >&2; return 1; }
    [ -n "$VMLINUZ" ] && [ -n "$INITRD" ] || {
        echo "vm-ctl: boot files missing in $IMAGES/boot (rebuild the base image)" >&2
        return 1;
    }
    mkdir -p "$SHARE"
    rm -f "$PIDFILE"

    # A stale qemu (e.g. from a killed terminal command that escaped the
    # process-tree kill because it runs in its own systemd scope) would hold
    # the hostfwd port and/or the overlay open, silently breaking this start
    # (the new qemu dies on the port bind while wait_ssh connects to the
    # STALE guest). Fail fast with actionable guidance instead.
    if ss -tln 2>/dev/null | grep -q ":$SSHPORT "; then
        echo "vm-ctl: port $SSHPORT already in use — a stale qemu is running." >&2
        echo "vm-ctl: run: bash vm/harness/vm-ctl.sh stop; pkill -f qemu-system; rm -f $PIDFILE" >&2
        return 1
    fi
    if pgrep -f qemu-system >/dev/null 2>&1; then
        echo "vm-ctl: a qemu-system process is already running (stale VM)." >&2
        echo "vm-ctl: run: bash vm/harness/vm-ctl.sh stop; pkill -f qemu-system" >&2
        return 1
    fi

    # --- OOM protection (host safety) ---
    # The VM's qemu process is confined to its own cgroup with a hard memory
    # ceiling (MemoryMax) and swap ceiling (MemorySwapMax). If a guest ever
    # exceeds the cap, the KERNEL oom-kills the qemu process inside that
    # cgroup — the host never reaches global OOM (which previously killed
    # unrelated host processes such as the editor). The cap is deliberately
    # above the VM's real footprint: qemu allocates guest RAM lazily and an
    # idle court VM uses ~1 GiB; the ceiling only trips on runaway growth.
    #
    # Tuning (documented in vm/README.md, "OOM protection"):
    #   KM_VM_MEM      guest RAM size passed to qemu      (default 8G)
    #   KM_VM_MEM_MAX  cgroup MemoryMax for qemu          (default 12G)
    #   KM_VM_SWAP_MAX cgroup MemorySwapMax for qemu      (default 4G)
    #
    # systemd-run --user is used when the user manager is available (cgroup
    # v2 memory controller); otherwise qemu runs uncapped with a warning.
    local VM_MEM="${KM_VM_MEM:-8G}"
    local VM_MEM_MAX="${KM_VM_MEM_MAX:-12G}"
    local VM_SWAP_MAX="${KM_VM_SWAP_MAX:-4G}"

    local qemu_args=(
        -enable-kvm -cpu host -smp 8 -m "$VM_MEM"
        -kernel "$VMLINUZ" -initrd "$INITRD"
        -append "$KERNEL_CMDLINE"
        -drive file="$overlay",if=virtio,format=qcow2
        -netdev user,id=n1,hostfwd=tcp:127.0.0.1:$SSHPORT-:22
        -device virtio-net-pci,netdev=n1
        -device virtio-balloon-pci
        -virtfs local,path="$SHARE",mount_tag=hostshare,security_model=none
        -display none -serial none -monitor none
        -pidfile "$PIDFILE"
        "$@"
    )

    # qemu runs in the foreground (writing vm.pid itself via -pidfile); the
    # whole invocation is backgrounded by us. NOTE: the pidfile must come
    # from qemu, NOT from `echo $$` in a shell — systemd-run serializes its
    # command line into a transient unit and its ExecStart re-parsing
    # collapses `$$` into a literal `$` (systemd's escape rule), so a shell
    # `$$` would write the string `$`. `-pidfile` avoids that entirely.
    # `is-system-running` reports "degraded" (nonzero) on desktop systems
    # with user units failing — treat running/degraded/starting as usable.
    local user_systemd=""
    if command -v systemd-run >/dev/null 2>&1; then
        local is_running
        is_running="$(systemctl --user is-system-running 2>/dev/null || true)"
        case "$is_running" in
            running|degraded|starting) user_systemd=1 ;;
        esac
    fi
    if [ -n "$user_systemd" ]; then
        systemd-run --user --scope --quiet \
            -p "MemoryMax=$VM_MEM_MAX" -p "MemorySwapMax=$VM_SWAP_MAX" \
            bash -c 'exec "$@"' _ qemu-system-x86_64 "${qemu_args[@]}" &
    else
        echo "vm-ctl: WARNING no usable user systemd; starting qemu WITHOUT cgroup memory cap" >&2
        bash -c 'exec "$@"' _ qemu-system-x86_64 "${qemu_args[@]}" &
    fi

    # give qemu a moment to write the pidfile, then wait for ssh
    local pid=""
    for _ in 1 2 3 4 5; do
        [ -f "$PIDFILE" ] && pid="$(cat "$PIDFILE")" && [ -n "$pid" ] && break
        sleep 1
    done
    echo "vm-ctl: waiting for ssh..."
    wait_ssh
    # ensure the 9p share is mounted in the VM
    ssh_run "mkdir -p /mnt/host && (mountpoint -q /mnt/host || mount -t 9p -o trans=virtio,version=9p2000.L hostshare /mnt/host)"
    echo "vm-ctl: ready (pid $(cat "$PIDFILE" 2>/dev/null || echo unknown))"
}

cmd_exec() {
    [ -f "$PIDFILE" ] || { echo "vm-ctl: vm not running" >&2; return 1; }
    ssh_run "$@"
}

cmd_put() {
    [ -f "$PIDFILE" ] || { echo "vm-ctl: vm not running" >&2; return 1; }
    local src="$1" dst="$2"
    scp -q "${SSHOPTS[@]}" "$src" "root@127.0.0.1:$dst"
}

cmd_stop() {
    [ -f "$PIDFILE" ] || { echo "vm-ctl: not running"; return 0; }
    ssh_run "poweroff" >/dev/null 2>&1 || true
    local pid; pid="$(cat "$PIDFILE")"
    for _ in $(seq 1 90); do
        kill -0 "$pid" 2>/dev/null || { rm -f "$PIDFILE"; echo "vm-ctl: stopped"; return 0; }
        sleep 1
    done
    echo "vm-ctl: forcing qemu shutdown" >&2
    kill "$pid" 2>/dev/null || true
    rm -f "$PIDFILE"
}

cmd_cleanup() {
    # Kill any stale qemu from killed terminal commands (they run in their
    # own systemd scopes and can survive process-tree kills), release the
    # hostfwd port, and remove a stale pidfile. Safe to run any time.
    pkill -9 -f 'qemu-system' >/dev/null 2>&1 || true
    sleep 1
    rm -f "$PIDFILE"
    echo "vm-ctl: cleaned stale qemu + pidfile"
}

case "${1:-}" in
    start) shift; cmd_start "$@" ;;
    exec) shift; cmd_exec "$@" ;;
    put) shift; cmd_put "$@" ;;
    stop) shift; cmd_stop "$@" ;;
    cleanup) shift; cmd_cleanup "$@" ;;
    *) echo "usage: $0 start|exec|put|stop|cleanup" >&2; exit 2 ;;
esac
