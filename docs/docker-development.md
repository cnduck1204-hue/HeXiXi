# Docker 开发约定

编译、用户程序镜像生成、QEMU 启动和冒烟测试全部在 Linux 容器内执行。
宿主机只负责编辑文件和运行 Docker，不要求本机安装 Rust、QEMU 或 make。

## 首次使用

1. 安装 Git 和 Docker。Windows、macOS 使用 Docker Desktop；Linux 安装 Docker Engine
   及 Compose 插件。确认 `docker info` 和 `docker compose version` 能正常执行。
2. 拉取项目，在包含 compose.yaml 的 HeXiXi 根目录执行 `docker compose build dev`。
   需要联网获取基础镜像、编译 QEMU 并安装 Rust 及辅助工具。
3. 执行 `docker compose run --rm dev "make test"`，检查能否启动并运行 hello_world。
4. 执行 `docker compose run --rm dev "make run"` 进入交互式 Shell。
   退出 QEMU 时先按 Ctrl+A，松开后按 X。

Compose 使用仓库内的 Dockerfile 构建 rcore-tutorial-v3:latest。
该名称是本地镜像标签，不表示远端仓库已发布了可直接拉取的团队镜像。
容器只挂载当前项目目录，不依赖工作区旁边的 rcore、NoAxiom 或 images。
当前启动使用 user 目录生成的 easy-fs 镜像，官方测例镜像不参与基本启动测试。

已有可用开发镜像的成员也可以导出镜像交给新电脑，省去重新构建工具链：

```sh
docker image save -o hexixi-dev-amd64.tar rcore-tutorial-v3:latest
```

新电脑取得该文件后执行 `docker image load -i hexixi-dev-amd64.tar`，
再按上述命令运行启动测试。镜像归档单独传递，不放入 Git 仓库。
这种方式只准备了开发工具；首次编译仍需联网下载项目依赖，除非另行准备完整依赖缓存。

## macOS

所有宿主机统一使用 linux/amd64 开发镜像，避免团队同时维护两套容器工具链。
M 系列 Mac 使用 Docker Desktop 的 amd64 兼容执行能力，编译速度可能较慢。
内核依旧编译为 RISC-V64，由容器内的 QEMU 模拟运行；不需要宿主机支持 RISC-V。
若 Docker 提示无法执行 amd64 程序，应先检查 Docker Desktop 的跨架构执行设置。

源码应放在 Docker Desktop 允许共享的目录中。挂载目录路径有空格时，使用 Compose
入口即可，无需手工拼接 docker run 的挂载参数。

## 缓存与离线使用

Cargo 缓存保存在 hexixi-cargo-registry 和 hexixi-cargo-git 两个 Docker 卷中。
编译产物保存在当前项目的 target 目录。普通容器退出后，这些数据仍保留。

依赖已完整缓存时：

```sh
docker compose run --rm -e CARGO_NET_OFFLINE=true dev "make test"
```

新电脑第一次运行不能直接依赖离线模式。缺少依赖时，去掉离线环境变量并联网重试。
修改 Dockerfile 或 rust-toolchain.toml 后，应重新构建开发镜像并再次执行启动测试。

## 验证范围

2026-11-16，Windows Docker、linux/amd64 环境完成以下验证：

- `docker compose config --quiet` 通过。
- `docker compose build dev` 成功，实际下载 Ubuntu 基础镜像、编译 QEMU 并安装 Rust 工具链。
- 新镜像中的工具版本为 rustc 1.100.0、QEMU 7.0.0。
- 使用新镜像执行 `docker compose run --rm -T -e CARGO_NET_OFFLINE=true dev "make test"`
  成功，确认内核及用户程序重新编译、中文标识输出、Shell 启动、hello_world 执行及返回提示符。
  该次内核构建复用了项目依赖下载缓存。

排查过程中，docker/dockerfile:1 与普通 ubuntu:20.04 下载均出现元数据大小校验错误
（1691 != 1324）；直接从 Docker 官方 registry-1.docker.io 拉取 Ubuntu 成功。
Dockerfile 因此移除了可选的外部解析器声明，使用 Docker 内置解析器，
并明确从该官方地址下载 Ubuntu 基础镜像。宿主机的 Docker 全局设置未修改。
这绕过了本次异常下载路径，宿主机其他镜像下载仍需单独检查。

后续构建曾在安装辅助工具之后，再次同步 stable 工具链时遇到 USTC 镜像域名的临时
DNS 解析失败。Dockerfile 已改为先通过 `rustup toolchain install` 读取项目的
rust-toolchain.toml，统一安装工具链、组件和目标，再单独安装 cargo-binutils。
去掉重复同步和不再需要的 toml-cli，两个安装步骤分别缓存；首次安装仍需正常的网络与 DNS。
调整后重新构建镜像成功，并再次通过启动冒烟测试；在禁用容器网络的检查中，
也确认 RISC-V 编译目标及 rust-src、LLVM 工具、rustfmt、clippy 组件已安装完整。

macOS Intel 与 Apple Silicon 尚未进行实机验证。

Docker 的跨架构执行机制参见
[官方多平台说明](https://docs.docker.com/build/building/multi-platform/)，
一次性开发容器命令参见
[docker compose run](https://docs.docker.com/reference/cli/docker/compose/run/)。
