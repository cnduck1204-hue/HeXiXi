#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
exec qemu-system-riscv64 \
    -machine virt -m 128M -smp 1 \
    -bios bootloader/rustsbi-qemu.bin \
    -serial mon:stdio -display none \
    -device loader,file=os/target/riscv64gc-unknown-none-elf/release/os.bin,addr=0x80200000 \
    -drive file=user/target/riscv64gc-unknown-none-elf/release/fs.img,if=none,format=raw,id=x0 \
    -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.7 \
    -device virtio-gpu-device,bus=virtio-mmio-bus.6 \
    -device virtio-keyboard-device,bus=virtio-mmio-bus.5 \
    -device virtio-mouse-device,bus=virtio-mmio-bus.4 \
    -device virtio-net-device,netdev=net0,bus=virtio-mmio-bus.3 \
    -netdev user,id=net0
