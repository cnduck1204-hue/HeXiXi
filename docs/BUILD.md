# 构建与运行

本文档只讲「怎么把它跑起来」。改动原因与环境细节见 [deploy-notes.md](deploy-notes.md)。

## 支持环境

- Ubuntu 22.04 / 24.04（原生或 WSL2）
- macOS 亦可，但需要自行安装 `riscv64-elf-gcc` 与 QEMU
- 不支持原生 Windows（Makefile 与工具链假定 POSIX 环境），请用 WSL2

## 一次性安装

```bash
sudo apt update
sudo apt install -y build-essential curl git make \
  qemu-system-misc gcc-riscv64-linux-gnu gcc-riscv64-unknown-elf \
  gdb-multiarch binutils-riscv64-unknown-elf

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

`rust-toolchain.toml` 会自动拉取 `nightly-2022-08-05` 与所需组件（`rust-src`、`llvm-tools-preview`），无需手动安装。

### crate 源（中国大陆网络必需）

`~/.cargo/config.toml`：

```toml
[source.crates-io]
replace-with = 'ustc-sparse'

[source.ustc-sparse]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"

[net]
git-fetch-with-cli = true

[unstable]
sparse-registry = true      # cargo 1.64 需要显式开启
```

### git 依赖（原始 GitHub 地址不可达时）

```bash
git config --global url."https://gitee.com/".insteadOf "https://github.com/"
```

> 注意：本仓库的 `os/Cargo.toml` 与 `user/Cargo.toml` 已经固化为 Gitee 地址，
> 上面这条只影响其它隐式引用 GitHub 的传递依赖。

## 编译与运行

```bash
cd os
make run            # 编译内核 + 用户程序 + 打包 fs.img，然后启动 QEMU
make run TEST=1     # 以 usertests 作为 initproc，开机自动跑全套测试
make build          # 只编译
make clean          # 清理产物
make debug          # 起 QEMU 等待 gdb（需要 tmux）
```

产物位置：

| 产物 | 路径 |
|---|---|
| 内核镜像 | `os/target/riscv64gc-unknown-none-elf/release/os.bin` |
| 文件系统镜像 | `user/target/riscv64gc-unknown-none-elf/release/fs.img` |

## 恢复默认 shell

`make run TEST=1` 会覆盖 `initproc`。想回到交互 shell：

```bash
cd user
rm -f target/riscv64gc-unknown-none-elf/release/initproc*
touch src/bin/initproc.rs
cd ../os && make build
```

## 常见问题

| 现象 | 原因 | 处理 |
|---|---|---|
| `Updating crates.io index` 长时间无响应 | 走的是 git 索引 | 配 USTC sparse 源（见上） |
| `usage of sparse registries requires -Z sparse-registry` | cargo 1.64 未开启开关 | `[unstable] sparse-registry = true` |
| `cannot be built because it requires rustc 1.xx or newer` | 依赖选了新版 | `Cargo.lock` 已锁定；若被删，重新 `cargo update -p regex --precise 1.7.3` |
| `riscv64-linux-musl-gcc: not found` | 用户程序 Makefile 的默认编译器不存在 | 本仓库已改用 Rust 编写，无需该编译器 |
| 启动后无任何串口输出 | 仓库自带 `rustsbi-qemu.bin` 损坏 | 已改为 `-bios default` + `-kernel` |
| 输入命令时首字符丢失 | 管道一次性灌入时的 UART 时序问题 | 用终端交互输入即可 |

## 版本锁定策略

本仓库提交了三个 `Cargo.lock`（`os/`、`user/`、`easy-fs-fuse/`），这是刻意的：
工具链固定在 2022 年的 nightly，而 crates.io 上的依赖持续升级，不锁版本随时会因 MSRV 失败。
**修改依赖后请一并提交 lockfile。**
