# rCore 来源

- 来源仓库：https://github.com/rcore-os/rCore-Tutorial-v3.git
- 本地参考目录：`../rcore`
- 来源提交：`c91bd3752b53ff48555aef4e3c7b8d5ddc8ee6e1`
- 导入日期：2026-11-16
- 许可证：保留根目录的上游 `LICENSE`（GPL-3.0）。

导入 os、user、easy-fs、easy-fs-fuse、bootloader 的已跟踪文件，
以及 Dockerfile、.dockerignore 和 rust-toolchain.toml。
原始 README 保存在 rcore-readme.md，原有文件中的作者信息保留。
同时保留本地 rCore 的 os、user、easy-fs-fuse 中现有 Cargo.lock，固定依赖解析结果。
本地 rCore 的编辑器配置改动未导入，未复制其 Git 历史和构建产物。

核夕夕自行添加团队说明、构建入口和启动冒烟测试。
源码改动包括内核及 Shell 标识、串口 UTF-8 字节输出。
后续导入上游变更时，记录来源提交及本地冲突处理。
