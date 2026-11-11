# RUST_CORE

一个用 Rust 从零实现的 RISC-V 64 位类 Unix 操作系统内核，基于
[rCore-Tutorial v3](https://github.com/rcore-os/rCore-Tutorial-v3)（ch8 分支）构建，
并针对 2026 年的工具链环境做了可复现的构建适配。

目标不是做一个生产级系统，而是把「操作系统到底怎么跑起来的」压缩到约 4500 行可读代码里。

## 特性

| 子系统 | 说明 |
|---|---|
| 硬件平台 | QEMU `virt` 机器，RISC-V 64，Sv39 三级页表 |
| 固件 | OpenSBI（QEMU 内置 `-bios default`） |
| 进程 / 线程 | `fork` / `exec` / `waitpid`，一进程多线程 + 信号 |
| 调度 | 单核 FIFO 轮转，100 Hz 时钟中断 |
| 内存 | 物理页帧分配器、内核堆、按需映射的地址空间 |
| 文件系统 | `easy-fs`：超级块 + 位图 + inode + 数据区，带块缓存 |
| 设备 | virtio-blk（MMIO）块设备驱动 |
| IPC | 管道、互斥锁、信号量、条件变量 |
| 用户态 | 27 个系统调用 + 45 个测试程序 + 一个交互 shell |

## 快速开始

```bash
# 依赖：Rust（rustup）、QEMU、make、git，见 docs/BUILD.md
git clone git@github.com:wonder-world/RUST_CORE.git
cd RUST_CORE/os

make run            # 编译 + 启动 QEMU，进入交互 shell
make run TEST=1     # 开机自动跑完整 usertests
```

启动后应当在串口看到：

```
OpenSBI v1.3
...
[kernel] Hello, world!
...
Rust user shell
>>
```

## 与原版的差异

上游代码停留在 2022 年的工具链，直接构建会因为网络与依赖版本问题失败。本仓库的改动：

| 类别 | 改动 |
|---|---|
| 依赖源 | GitHub → Gitee 镜像；crates.io → USTC sparse 镜像 |
| 依赖版本 | `log` 钉 0.4.17、`regex` 钉 1.7.3、`libc` 钉 0.2.160（适配 rustc 1.64） |
| 构建 | 宿主工具 `easy-fs-fuse` 改用 stable 编译；`rust-objcopy` 由 `llvm-tools` 提供 |
| 启动 | 弃用仓库内损坏的 `rustsbi-qemu.bin`，改用 `-bios default` + `-kernel` |
| 裁剪 | `sdcard`（K210）驱动按 feature 门控，QEMU 构建不再依赖 k210 crate |

完整的踩坑记录见 [docs/deploy-notes.md](docs/deploy-notes.md)。

## 目录结构

```
├── os/                 内核本体
│   ├── src/
│   │   ├── main.rs     入口（rust_main）
│   │   ├── entry.asm   上电第一条指令
│   │   ├── trap/       陷阱：保存现场 → 分派 → 返回
│   │   ├── mm/         页表、地址空间、帧/堆分配
│   │   ├── task/       进程、线程、调度、上下文切换
│   │   ├── syscall/    27 个系统调用
│   │   ├── fs/         文件抽象、fd 表、管道、stdio
│   │   ├── drivers/    virtio-blk 驱动
│   │   ├── sync/       互斥锁、信号量、条件变量
│   │   └── boards/     板级常量（qemu / k210）
│   └── Makefile        编译 + QEMU 启动参数
├── user/               用户态：库 + 45 个程序（src/bin/）
├── easy-fs/            与 OS 无关的文件系统库
├── easy-fs-fuse/       打包工具，生成 fs.img
├── bootloader/         预编译固件（保留作参考）
└── docs/               构建说明、部署记录、上游文档
```

## 环境要求

| 组件 | 版本 |
|---|---|
| Rust | 仓库 `rust-toolchain.toml` 锁定 `nightly-2022-08-05` |
| QEMU | ≥ 5.0（推荐 8.x，需含 `qemu-system-riscv64`） |
| 系统 | Linux / WSL2（Ubuntu 24.04 验证通过） |

详细安装步骤见 [docs/BUILD.md](docs/BUILD.md)。

## 学习路径

按分支切换，可以对照教程逐章读：

```bash
git log --oneline --all --decorate | head -20   # 查看各章提交
git checkout ch4                                 # 只看到第 4 章的程度
git diff ch3 ch4                                 # 看这一章加了什么
```

原版教程：[rCore-Tutorial-Book-v3](https://rcore-os.github.io/rCore-Tutorial-Book-v3/)

## 许可证

沿用上游，见 [LICENSE](LICENSE)。
