# 对接 oscomp 官方评测

## 评测机的行为

评测机在仓库根目录执行 `make all`，然后启动：

```bash
qemu-system-riscv64 -machine virt -kernel kernel-rv -m <mem> -nographic -smp <smp> \
  -bios default -drive file={fs},if=none,format=raw,id=x0 \
  -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 -no-reboot \
  -device virtio-net-device,netdev=net -netdev user,id=net -rtc base=utc \
  -drive file=disk.img,if=none,format=raw,id=x1 \
  -device virtio-blk-device,drive=x1,bus=virtio-mmio-bus.1
```

其中 `{fs}` 是官方提供的 **EXT4 镜像**（无分区表，4 GB），根目录下是编译好的
ELF 和 `xxx_testcode.sh` 脚本。内核需要在启动后：

1. 扫描该磁盘的根目录；
2. 依次串行执行每个 `*_testcode.sh`；
3. 输出形如 `#### OS COMP TEST GROUP START basic ####` 的标记（脚本里自带）；
4. 全部跑完后主动关机。

评分依据是屏幕输出。

## 我们目前的状态

| 项 | 状态 |
|---|---|
| `make all` 产出 `kernel-rv` | ✅ 根目录 Makefile 已实现 |
| basic 组用例 | ✅ 本地 26/26 全通过（用 easy-fs 打包，脚本手动触发） |
| 扫描官方 EXT4 测试盘 | ❌ 未实现 |
| 自动执行脚本、输出标记、关机 | ❌ 未实现 |
| 其余 11 组（busybox/lua/libc-test/…） | ❌ 未测 |

## 关键阻塞：EXT4

rCore 自带的是 `easy-fs`，磁盘布局完全不同：

```
easy-fs:  [超级块][inode 位图][数据位图][inode 区][数据区]
EXT4:     [超级块][块组描述符][块位图][inode 位图][inode 表][数据块]
```

内核现在把 bus.0 上的磁盘当 easy-fs 解析，遇到 EXT4 镜像会在
`EasyFileSystem::open` 处 panic，因此拿不到任何分数。

### 三条可选路线

| 路线 | 做法 | 工作量 |
|---|---|---|
| A. 实现 EXT4 只读驱动 | 解析超级块、块组描述符、extent 树、目录项；只需读 | 大（约 1.5–3k 行） |
| B. 让官方脚本跑在 easy-fs 上 | 评测机给的是 EXT4，无法在构建期转换 | 不可行 |
| C. 双文件系统 | 识别 magisk 魔数后分别走 easy-fs / ext4 | 等于 A + 少量胶水 |

结论：想拿到官方分数，A 是唯一路径。

## 建议的推进顺序

1. **先把 basic 的 26 个用例做成开机自动跑**（用 easy-fs 那张盘）：
   内核在 `rust_main` 里遍历根目录的 `t_*`，逐个 fork+exec，最后关机。
   这一步能跑通“自动执行 + 输出标记 + 关机”的完整链路，与 EXT4 解耦。
2. **再写 EXT4 只读驱动**（P0：超级块 + 块组 + inode + extent + 目录项）。
3. 把 1 的遍历逻辑改为扫描 EXT4 根目录里的 `*_testcode.sh`，用 shell 执行脚本。
4. 逐步扩展到 busybox / lua / libc-test。

## 本地复现

```bash
tools/oscomp/build_tests.sh      # 编译 26 个用例并打包 fs.img
cd os && make run                # 启动，在 shell 里逐个执行 t_xxx
```
