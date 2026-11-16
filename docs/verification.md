# 首次验证记录

日期：2026-11-16。

环境：Windows PowerShell + Docker，开发镜像 rcore-tutorial-v3:latest，
镜像内 rustc 1.99.1、QEMU 7.0.0，目标 riscv64gc-unknown-none-elf。

执行 `./dev.ps1 test -Offline`，退出码为 0。确认：

- 内核、用户程序及 easy-fs 镜像打包工具编译成功。
- QEMU 启动并正确输出 UTF-8 标识 `核夕夕 HeXiXi | based on rCore`。
- 进入 `HeXiXi user shell`。
- 自动输入 hello_world，获得 `Hello world from user mode program!`，并返回提示符。

测试结果：`PASS: HeXiXi banner, user shell, hello_world, return to shell`。
在添加核夕夕标识之前，同一测试启动到 rCore Shell 后以
`Missing HeXiXi kernel banner` 失败，确认测试确实检查新项目标识。

首次构建曾遇到下载域名解析失败；复用本机 Cargo 缓存并补齐依赖后完成验证，
后续离线构建所需缓存保存在 Docker 卷中。

核对导入文件：只有 os/src/main.rs、os/src/console.rs、
user/src/bin/user_shell.rs 三个已有源码文件不同于来源目录。
LICENSE 与来源文件一致。原 rcore 目录未被修改。

验证范围为基本启动与用户程序执行，不涵盖全部 rCore 测试或官方赛事测例。
