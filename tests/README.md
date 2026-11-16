# 测试

参考 NoAxiom 的 official/custom 划分，当前目录如下：

```text
tests/
├── official/
│   ├── src/          # testsuits-for-oskernel 源码快照
│   ├── img/          # 官方 RISC-V 测试镜像及压缩包
│   ├── SOURCES.json  # 源码版本、镜像大小与 SHA-256
│   └── README.md     # 来源和使用范围
├── custom/           # 团队自建测例
├── boot.py           # 现有 QEMU 启动冒烟测试
└── README.md
```

官方源码与镜像目前仅暂存。`make test` 继续运行启动冒烟测试；
官方测例尚未接入内核，不能直接以官方 Ext4 镜像替换当前 easy-fs 启动盘。
官方镜像体积较大，已从 Git 跟踪中排除；源码快照和来源说明可正常提交。

## 启动冒烟测试

在项目根目录执行 `docker compose run --rm dev "make test"`，
或在 Windows 执行 `./dev.ps1 test`；`make test` 由容器执行。
测试会先构建内核和 easy-fs 用户程序镜像，然后启动 QEMU，检查核夕夕启动标识、
进入用户 Shell、执行 `hello_world` 并返回 Shell。60 秒内未完成则报错并结束 QEMU。

自动测试使用 Linux 的管道等待接口，Windows 请通过上述 Docker 入口运行。
这只是启动冒烟测试，不代表已通过官方赛事测例。
