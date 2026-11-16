# 官方测例暂存

## 源码

`src/` 从工作区现有的 testsuits-for-oskernel 复制已跟踪文件，
保留原目录结构、说明和许可证文件，不复制其 .git 历史或本地编译产物。

- 上游：https://github.com/oscomp/testsuits-for-oskernel.git
- 来源提交：`8b58dd16d26d30f7c74d48d5832d870d3051b703`
- 源码文件数：14473

这是一次本地源码快照，不是 Git 子模块，不会自动跟随上游更新。
测试内容及构建方法见 src/README.md。

首次将快照提交到团队仓库时，以下上游已跟踪文件会被其原有忽略规则隐藏，
需显式加入（在 HeXiXi 根目录执行）：

```sh
git add -f tests/official/src/busybox/Config.in tests/official/src/ltp-full-20240524/VERSION
git add -f tests/official/src/sdcard/loongarch/.gitkeep tests/official/src/sdcard/riscv/.gitkeep
```

快照中还有上游已跟踪的 rt-tests-2.7/__pycache__ 缓存文件，已复制到本地，
但按核夕夕现有规则排除出 Git 提交。

## 镜像

`img/` 复制工作区 images 中现有的两个文件：

- sdcard-rv.img：解压后的 RISC-V 官方测例镜像，4 GiB。
- sdcard-rv.img.xz：同目录已有的压缩包。

复制日期、文件大小、SHA-256 校验值见 SOURCES.json。
记录的是本地来源和复制完整性，未重新下载或核实镜像所属的赛事发布版本。
这两个大文件仅保存在本地，已通过项目根目录 .gitignore 排除，避免提交到团队仓库。
其他成员需自行取得文件并放入 img/，再按 SOURCES.json 核对校验值。

## 当前范围

本次只整理存放，未挂载官方镜像、未编译官方测例，也未修改内核或启动入口。
当前 rCore 基线使用 easy-fs；后续接入官方 Ext4 镜像还需单独完成文件系统和运行接口适配。
外部 testsuits-for-oskernel 与 images 原件保留。
