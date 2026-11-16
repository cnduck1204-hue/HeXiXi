# 团队开发约定

## 模块分工

| 范围 | 主要目录 |
| --- | --- |
| 启动、异常、中断、设备 | os/src/boards、drivers、trap、entry.asm |
| 内存管理 | os/src/mm |
| 进程、线程、调度、同步 | os/src/task、sync、timer.rs |
| 文件系统与 I/O | os/src/fs、easy-fs |
| 系统调用与用户态接口 | os/src/syscall、user/src/lib.rs |
| 网络 | os/src/net |
| 工具与验证 | easy-fs-fuse、tools、tests |
| 设计与协作记录 | docs |

按模块领取任务；改动系统调用、共享结构或跨模块接口前，先在 docs 中写清调用方式、
数据约定及受影响模块，再与相关开发者确认。

## 提交与合并

- 在功能分支开发，例如 feat/process 或 fix/page-fault，保持每次提交只解决一个问题。
- 保留现有编码风格，不随手重构相邻模块或删除上游代码。
- 修改 Rust 文件前后检查格式；不要把无关的整库格式变化混入提交。
- 提交 Cargo.lock，使团队共享依赖解析结果；不提交 target、镜像或日志。
- 变更说明写清触发场景、实际行为、验证结果和已知限制。
- 合并前运行 `docker compose run --rm dev "make test"` 或 Windows 的 `./dev.ps1 test`；
  模块行为变化还需附对应复现或测试。编译和运行均在 Docker 内完成。

启动测试只覆盖串口、Shell 和用户程序基本链路。涉及内存、进程、系统调用或文件系统时，
应增加能覆盖该改动的用户程序测试；赛事兼容性通过后续专项测试确认。
