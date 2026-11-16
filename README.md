# 核夕夕 HeXiXi

基于 rCore-Tutorial-v3 的团队内核开发项目，首版目标为 RISC-V64 + QEMU。
目录职责参考 NoAxiom，启动与内核实现沿用本地 rCore 基线。

## 目录

```text
HeXiXi/
├── os/               # 内核：内存、调度、系统调用、驱动等
├── user/             # 用户库、Shell 和用户程序
├── easy-fs/          # 内核与镜像工具共享的文件系统库
├── easy-fs-fuse/     # 用户程序文件系统镜像打包工具
├── bootloader/       # rCore 配套 RustSBI 固件
├── tools/            # QEMU 启动工具
├── tests/            # 官方测例源码与镜像、自建测例、启动测试
├── docs/             # 设计、来源与开发记录
├── Makefile          # 容器内统一入口
├── compose.yaml      # Windows、Linux、macOS 通用 Docker 入口
└── dev.ps1           # Windows Docker 统一入口
```

保留 rCore 的 `os/` 命名及相对路径，使现有配套组件可以直接协同构建。
公共库暂沿用现有目录，待形成实际模块边界后再拆分。

## 新电脑初始化（Windows、Linux、macOS）

团队统一在 Docker 内编译和运行，宿主机只需 Git、Docker 和 Docker Compose，
不需要安装 Rust、QEMU 或 make。Windows、macOS 使用 Docker Desktop 的 Linux 容器环境；
Linux 使用 Docker Engine 和 Compose 插件。

拉取项目后，在包含 compose.yaml 的 HeXiXi 根目录执行：

```sh
docker compose build dev
docker compose run --rm dev "make test"
docker compose run --rm dev "make run"
```

第一条命令根据仓库 Dockerfile 构建开发镜像，需要联网下载基础镜像、QEMU 和 Rust 工具链，
耗时会长于日常构建。已有同名开发镜像时可以跳过第一条。
项目源码通过目录挂载进入容器，构建产物保存在项目 target 目录；
Cargo 下载缓存在 Docker 卷中，无需把其他电脑的缓存复制过来。

当前已在 Windows Docker 完成开发镜像构建和新镜像启动测试。
macOS 实机尚未验证，详细记录见 [Docker 开发说明](docs/docker-development.md)。

## Windows 便捷入口

完成镜像初始化后，也可在此项目目录打开 PowerShell：

```powershell
./dev.ps1 build
./dev.ps1 run
./dev.ps1 test
```

`run` 和 `test` 都会先增量构建。构建输出留在当前项目目录，不依赖旁边的 rcore。
首次构建需要下载 Rust 依赖，后续复用 Docker 的 hexixi-cargo-registry 和
hexixi-cargo-git 缓存卷；依赖已缓存时可加 `-Offline`，例如 `./dev.ps1 test -Offline`。
启动后可看到 `核夕夕 HeXiXi | based on rCore`，
随后进入 `HeXiXi user shell`，输入 `hello_world` 验证用户程序执行。
退出 QEMU：先按 Ctrl+A，松开后按 X。

此阶段仅使用项目自身生成的 easy-fs 镜像，不需要挂载外部赛事镜像。

## 日常 Docker 开发

Windows、Linux、macOS 使用相同命令：

```sh
docker compose run --rm dev "make build"
docker compose run --rm dev "make run"
docker compose run --rm dev "make test"
```

开发镜像统一指定 linux/amd64。Intel Mac 使用同架构容器，M 系列 Mac 需要
Docker 的 amd64 兼容执行能力；这不改变内核的 RISC-V64 目标架构。
macOS 实机尚未验证，不能视为已完成兼容性验收。

首次构建和依赖下载完成后，离线开发可执行：

```sh
docker compose run --rm -e CARGO_NET_OFFLINE=true dev "make test"
```

离线模式需要本机已具备开发镜像、所需工具链和全部依赖缓存。

## 团队开发

模块边界、提交及验证约定见 [CONTRIBUTING.md](CONTRIBUTING.md)。
当前为 rCore 衍生开发基线，尚未适配官方赛事测例，也不支持 LoongArch。
来源与版本见 [docs/upstream.md](docs/upstream.md)，原始 README 见
[docs/rcore-readme.md](docs/rcore-readme.md)。保留上游 GPL-3.0 许可证和作者署名。
