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
| 从 EXT4 盘启动（扫描、读文件、执行） | ✅ 只读 EXT4 驱动（超级块/块组/inode/extent/目录） |
| 解析 `*_testcode.sh` | ✅ 优先用盘里的 `busybox sh`，无 busybox 时用内置 mini shell |
| 输出 `#### OS COMP TEST GROUP ####` 标记 | ✅ |
| 跑完自动关机 | ✅（init 以 0 退出 → `exit_success`） |
| basic 组 | ✅ **32/32 全通过**（官方布局 EXT4 盘 + 真实 busybox sh） |
| lua 组 | ✅ **9/9 全通过**（runner 直接驱动 lua 解释器） |
| busybox 组 | ✅ 55 条命令中 **49 条成功**（剩 df/dmesg/ps/hwclock/which/后台 kill） |
| libcbench 组 | ⚠️ 能启动并输出标记，但 fork 出的子进程写栈缺页、多次 fork 后帧耗尽 |
| libc-test / ltp / iperf / netperf | ❌ 未测（iperf/netperf 依赖网卡，预期不可行） |
| cyclictest | ⚠️ 空载 2 个用例 success；hackbench 压力阶段在单核抢占下过慢、超时 |
| libcbench | ⚠️ 16 个 benchmark 有结果，卡在 2500 次 pthread create/join 循环 |
| lmbench | ⚠️ libtirpc 已能交叉构建；`lmbench_all` 还缺各子基准的 `*_main` 符号，未完成 |
| unixbench | ⚠️ 可产出 11–17 行结果（DHRY2/SYSCALL/CONTEXT/PIPE/SPAWN/9×fstime/3×SHELL），
但组尾会出现内核 panic（processor.rs / address.rs），且 FS_READ/COPY 测得 0 KBps |
| 信号支持 | ✅ `setitimer(103)` + `rt_sigaction(134)` + `rt_sigreturn(139)`，SIGALRM 可正常投递 |

## 复现方式

```bash
# 1) 构建用例二进制（basic 26 个 + test_echo）
tools/oscomp/build_tests.sh
# 2) 构建 busybox / lua
tools/oscomp/build_busybox.sh
tools/oscomp/build_lua.sh
# 3) 生成官方布局 EXT4 盘
tools/oscomp/make_official_image.sh
# 4) 启动
cd os && make build && qemu-system-riscv64 -machine virt -m 256M -nographic \
  -bios default -kernel target/riscv64gc-unknown-none-elf/release/os.bin \
  -drive file=$HOME/oscomp-work/official/disk.img,if=none,format=raw,id=x0 \
  -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0
```

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
