#!/usr/bin/env bash
#
# provision-rootfs.sh — runs INSIDE the privileged Docker builder container.
#
# Produces, in order:
#   1. /out/base.raw     — raw disk image (BIOS MBR + one ext4 root partition)
#   2. /out/base.qcow2   — qcow2 conversion of base.raw (the immutable base)
#   3. /out/manifest.json— reference_image_hash + full package manifest +
#                          oracle revision + build environment fingerprints
#
# The result is a controlled CachyOS userland with the ORACLE application
# built from the frozen commit, ready for snapshot-based courts.
#
# Determinism note: CachyOS is a rolling distribution; there are no
# versioned repo snapshots. Reproducibility is therefore snapshot-based:
# every court runs from a copy-on-write overlay of THIS image, and the
# manifest pins the exact package versions + image hash so any rebuild with
# different versions is detectable.
#
set -euo pipefail

mkdir -p /out /out/boot
LOG=/out/provision.log
exec > >(tee -a "$LOG") 2>&1

MIRROR="https://mirror.cachyos.org/repo/x86_64/cachyos"
ORACLE_COMMIT="6b4a373e6a4e7295a0803034e597c4f2a055a411"
ORACLE_TARBALL="/in/oracle-src.tar.gz"
IMG_ROOT=/img
OUT=/out
RAW_IMAGE="$OUT/base.raw"
QCOW_IMAGE="$OUT/base.qcow2"
DISK_SIZE=16G
ROOT_LABEL="cachyoskmroot"

step() { echo; echo "== [$(date -u +%H:%M:%S)] $*"; }

step "container basics: keyring + sync"
pacman-key --init
pacman-key --populate archlinux

# CachyOS keyring + mirrorlist packages, installed directly from the mirror
# (they cannot come from the repo before the keyring itself is trusted).
KEYRING_PKG="$(curl -fsSL "$MIRROR/" | grep -oE 'cachyos-keyring-[^"<]*\.pkg\.tar\.zst' | sort -u | head -1)"
MIRRORLIST_PKG="$(curl -fsSL "$MIRROR/" | grep -oE 'cachyos-mirrorlist-[^"<]*\.pkg\.tar\.zst' | sort -u | head -1)"
curl -fsSL -o /tmp/keyring.pkg.tar.zst "$MIRROR/$KEYRING_PKG"
curl -fsSL -o /tmp/mirrorlist.pkg.tar.zst "$MIRROR/$MIRRORLIST_PKG"
pacman -U --noconfirm /tmp/keyring.pkg.tar.zst /tmp/mirrorlist.pkg.tar.zst

# Mirrorlist for core/extra/multilib (Arch mirrors); the cachyos section has
# an explicit Server=, so /etc/pacman.d/mirrorlist only affects Arch repos.
# geo.mirror.pkgbuild.com is the primary; the others are fallbacks pacman
# tries per-file if the primary throttles or fails (seen during builds).
cat > /etc/pacman.d/mirrorlist <<'EOF'
Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch
Server = https://archlinux.mirror.wearetriple.com/$repo/os/$arch
Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch
EOF

pacman -Syu --noconfirm
pacman -S --noconfirm --needed arch-install-scripts util-linux e2fsprogs

step "pacstrap rootfs"
mkdir -p "$IMG_ROOT"
# linux-cachyos + headers double as the boot kernel AND the baseline
# discovery fixture. qemu-guest-agent is intentionally absent (unused
# surface); openssh is the harness transport; xorg-server provides Xvfb;
# at-spi2-core + python-gobject provide the accessibility observation path.
# No bootloader is installed: the VM boots via qemu -kernel/-initrd.
pacstrap -K "$IMG_ROOT" \
    base base-devel \
    linux-cachyos linux-cachyos-headers \
    sudo openssh \
    python python-pip dbus at-spi2-core python-gobject \
    xorg-server xorg-server-xvfb xorg-xrandr ttf-dejavu \
    strace procps-ng \
    polkit polkit-qt6 chwd scx-manager \
    qt6-base qt6-tools glib2 pkg-config \
    cmake ninja git rsync \
    cargo rust clang lld llvm \
    cachyos-keyring cachyos-mirrorlist

step "restore CachyOS pacman.conf in rootfs"
# pacstrap resolves the transaction with a TEMPORARY copy of the container's
# pacman.conf (which has [cachyos]) but installs the `pacman` package's own
# /etc/pacman.conf into the target — the stock Arch default with NO cachyos
# repo section (pacman owns that file; pacstrap does not copy the host conf
# unless -P). The oracle's libalpm then registers only core/extra, so
# linux-cachyos and every other cachyos package vanish from discovery.
# Restore the CachyOS config (cachyos repo FIRST, matching a real CachyOS
# /etc/pacman.conf section order — the oracle registers sync databases in
# pacman.conf order).
cp /etc/pacman.conf "$IMG_ROOT/etc/pacman.conf"

step "configure rootfs"
# --- fstab (root by label) ---
cat > "$IMG_ROOT/etc/fstab" <<EOF
# <device>    <dir>    <type>    <options>            <dump>  <pass>
LABEL=$ROOT_LABEL  /  ext4  rw,relatime  0 1
EOF

# --- hostname / hosts ---
echo "cachyos-km-vm" > "$IMG_ROOT/etc/hostname"
cat > "$IMG_ROOT/etc/hosts" <<'EOF'
127.0.0.1   localhost
127.0.1.1   cachyos-km-vm
EOF

# --- locale ---
sed -i 's/^#en_US.UTF-8/en_US.UTF-8/' "$IMG_ROOT/etc/locale.gen"
echo "LANG=en_US.UTF-8" > "$IMG_ROOT/etc/locale.conf"

# --- systemd-networkd: DHCP on the virtio NIC ---
mkdir -p "$IMG_ROOT/etc/systemd/network"
cat > "$IMG_ROOT/etc/systemd/network/20-wired.network" <<'EOF'
[Match]
Name=en*
[Network]
DHCP=yes
EOF

# --- serial console + harness services ---
mkdir -p "$IMG_ROOT/etc/systemd/system/serial-getty@ttyS0.service.d"
cat > "$IMG_ROOT/etc/systemd/system/serial-getty@ttyS0.service.d/override.conf" <<'EOF'
[Service]
Restart=always
EOF

# --- accessibility observation: vendored pyatspi2 over python-gobject ---
# pyatspi2 is not packaged in Arch; its dependency set is gi.repository.Atspi
# (at-spi2-core + python-gobject) + ctypes. We vendor the pure-python client
# directly so the image has no pip/pypi dependency at all.
arch-chroot "$IMG_ROOT" git clone -q --depth 1 https://gitlab.gnome.org/GNOME/pyatspi2.git /opt/a11y/pyatspi2

# --- GRUB: serial console, quiet boot ---
# (no bootloader in the image; qemu -kernel boot. This block is retained for
# reference only and is a no-op.)
# sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0"/' \
#     "$IMG_ROOT/etc/default/grub" 2>/dev/null || true
# cat >> "$IMG_ROOT/etc/default/grub" <<'EOF'
# GRUB_TERMINAL=serial
# GRUB_SERIAL_COMMAND="serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1"
# EOF

# --- harness user + ssh key (pubkey injected by the builder via arg) ---
arch-chroot "$IMG_ROOT" useradd -m -G wheel -s /bin/bash test || true
echo "test ALL=(ALL) NOPASSWD: ALL" > "$IMG_ROOT/etc/sudoers.d/test"
echo "root ALL=(ALL) NOPASSWD: ALL" > "$IMG_ROOT/etc/sudoers.d/root"
chmod 440 "$IMG_ROOT/etc/sudoers.d/"*
# passwordless ssh for root and test
if [ -n "${SSH_PUBKEY:-}" ]; then
    mkdir -p "$IMG_ROOT/root/.ssh" "$IMG_ROOT/home/test/.ssh"
    echo "$SSH_PUBKEY" > "$IMG_ROOT/root/.ssh/authorized_keys"
    echo "$SSH_PUBKEY" > "$IMG_ROOT/home/test/.ssh/authorized_keys"
    chmod 700 "$IMG_ROOT/root/.ssh" "$IMG_ROOT/home/test/.ssh"
    chmod 600 "$IMG_ROOT/root/.ssh/authorized_keys" "$IMG_ROOT/home/test/.ssh/authorized_keys"
    chown -R 0:0 "$IMG_ROOT/root/.ssh"
    chown -R 1000:1000 "$IMG_ROOT/home/test/.ssh"
fi
sed -i 's/^#PermitRootLogin.*/PermitRootLogin yes/' "$IMG_ROOT/etc/ssh/sshd_config"

# --- safety marker (directive §74): fail-closed gate for destructive courts ---
# machine-id class: the VM sets a distinctive /etc/machine-id at first boot;
# we additionally plant an unmistakable fixture marker file.
mkdir -p "$IMG_ROOT/etc/cachyos-km"
cat > "$IMG_ROOT/etc/cachyos-km/fixture.marker" <<'EOF'
# Unmistakable marker that this machine is an approved disposable court VM.
# Destructive and privileged courts MUST fail closed unless this file exists
# AND /etc/machine-id belongs to the approved class AND the snapshot identity
# matches the court harness. See docs/COURTS.md (host safety).
fixture_class = "cachyos-km-disposable-vm"
fixture_version = "1"
EOF

# --- polkit test rule: allow the oracle's action without password ---
mkdir -p "$IMG_ROOT/etc/polkit-1/rules.d"
cat > "$IMG_ROOT/etc/polkit-1/rules.d/10-cachyos-km-test.rules" <<'EOF'
// TEST-ONLY polkit rule. The oracle's own policy requires auth_admin; this
// rule authorizes the action for active sessions WITHOUT password so
// transaction courts can run unattended. It is part of the court fixture
// (never shipped to real systems).
polkit.addRule(function(action, subject) {
    if (action.id == "org.cachyos.KernelManager.pkexec.policy.run-root-terminal") {
        if (subject.isInGroup("wheel") || subject.user == "root") {
            return polkit.Result.YES;
        }
    }
});
EOF

# --- in-VM harness scripts ---
mkdir -p "$IMG_ROOT/opt/cachyos-km-vm"
# (copied below from /in/vm-in-vm after rootfs is finalized)

step "build oracle from frozen commit inside rootfs"
# 1) materialize the frozen source into the chroot (tarball has no top dir)
mkdir -p "$IMG_ROOT/build/oracle"
tar -xzf "$ORACLE_TARBALL" -C "$IMG_ROOT/build/oracle"

# 2) copy build script into chroot and run it there
cat > "$IMG_ROOT/build/build-oracle.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd /build/oracle
# CMake build; the official CachyOS recipe uses clang/lld with LTO
export CC=clang CXX=clang++ CARGO_FLAGS=""
cmake -S . -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DCMAKE_CXX_FLAGS="-Wno-error" \
    -DCMAKE_EXE_LINKER_FLAGS="-Wno-error"
cmake --build build -j"$(nproc)"
DESTDIR=/ cmake --install build
EOF
chmod +x "$IMG_ROOT/build/build-oracle.sh"
arch-chroot "$IMG_ROOT" /build/build-oracle.sh

step "verify oracle install inside rootfs"
arch-chroot "$IMG_ROOT" test -x /usr/bin/cachyos-kernel-manager
arch-chroot "$IMG_ROOT" /usr/bin/cachyos-kernel-manager --version 2>&1 | head -1 || true

# install in-VM harness scripts
mkdir -p "$IMG_ROOT/opt/cachyos-km-vm"
if [ -d /in/vm-in-vm ]; then
    cp -r /in/vm-in-vm/* "$IMG_ROOT/opt/cachyos-km-vm/"
    chmod +x "$IMG_ROOT/opt/cachyos-km-vm/"*.sh 2>/dev/null || true
fi

step "enable services"
arch-chroot "$IMG_ROOT" systemctl enable sshd systemd-networkd getty@ttyS0

step "create disk image (loop-free)"
# The image is built WITHOUT loop devices or mounts:
#   1. sparse raw file + MBR partition table,
#   2. mkfs.ext4 -d writes the rootfs DIRECTORY into a filesystem image,
#   3. dd places the fs image at the partition offset.
# The VM boots via qemu -kernel/-initrd (host files) with root=LABEL=, so no
# bootloader is written to the disk at all.
rm -f "$RAW_IMAGE"
truncate -s "$DISK_SIZE" "$RAW_IMAGE"
printf 'type=83, start=2048, bootable\n' | sfdisk "$RAW_IMAGE"

ROOTFS_IMG="$OUT/rootfs.img"
rm -f "$ROOTFS_IMG"
truncate -s 8G "$ROOTFS_IMG"
mkfs.ext4 -q -L "$ROOT_LABEL" -d "$IMG_ROOT" "$ROOTFS_IMG"
dd if="$ROOTFS_IMG" of="$RAW_IMAGE" bs=1M seek=1 conv=notrunc status=none
sync

# boot kernel + initramfs for qemu -kernel boot (host side)
mkdir -p "$OUT/boot"
cp "$IMG_ROOT"/boot/vmlinuz-* "$OUT/boot/"
cp "$IMG_ROOT"/boot/initramfs-*.img "$OUT/boot/"
ls -la "$OUT/boot/"

# export the rootfs DIRECTORY for loop-free fixture baking
step "export base rootfs directory"
rm -rf "$OUT/base-rootfs"
cp -a "$IMG_ROOT" "$OUT/base-rootfs"
chmod -R u+rwX "$OUT/base-rootfs"

step "write manifest (raw)"
RAW_HASH="$(sha256sum "$RAW_IMAGE" | awk '{print $1}')"
{
    echo "{"
    echo "  \"raw_image_hash\": \"sha256:$RAW_HASH\","
    echo "  \"oracle_commit\": \"$ORACLE_COMMIT\","
    echo "  \"builder_docker_image\": \"$BUILDER_IMAGE\","
    echo "  \"built_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
    echo "  \"packages\": ["
    arch-chroot "$IMG_ROOT" pacman -Q | sed 's/^/    "/; s/$/",/' | sed '$ s/,$//'
    echo "  ]"
    echo "}"
} > "$OUT/manifest.json"
chmod 644 "$OUT/manifest.json"
# NOTE: the host-side builder converts base.raw -> base.qcow2 and records
# reference_image_hash (the qcow2 digest) into the manifest and lock.

step "DONE"
ls -la "$OUT"
