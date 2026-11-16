use crate::fs::{open_file, OpenFlags};
use crate::mm::{translated_ref, translated_refmut, translated_str};
use crate::task::{
    current_process, current_task, current_user_token, exit_current_and_run_next, pid2process,
    suspend_current_and_run_next, SignalFlags,
};
use crate::timer::get_time_ms;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().process.upgrade().unwrap().getpid() as isize
}

pub fn sys_fork() -> isize {
    let current_process = current_process();
    let new_process = current_process.fork();
    let new_pid = new_process.getpid();
    // modify trap context of new_task, because it returns immediately after switching
    let new_process_inner = new_process.inner_exclusive_access();
    let task = new_process_inner.tasks[0].as_ref().unwrap();
    let trap_cx = task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let raw_path = translated_str(token, path);
    // exec 的路径同样要做归一化（busybox sh 传入的是 ./prog）
    let path = crate::fs::resolve_path(&raw_path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        if arg_str_ptr == 0 {
            break;
        }
        args_vec.push(translated_str(token, arg_str_ptr as *const u8));
        unsafe {
            args = args.add(1);
        }
    }
    if let Some(all_data) = crate::fs::read_file_all(path.as_str()) {
        let process = current_process();
        // 支持 shebang：把脚本交给解释器运行（Linux 的 binfmt_script 行为）
        if all_data.starts_with(b"#!") {
            let end = all_data.iter().position(|&c| c == b'\n').unwrap_or(all_data.len());
            let line = core::str::from_utf8(&all_data[2..end]).unwrap_or("").trim();
            let mut it = line.split_whitespace();
            let interp = it.next().unwrap_or("");
            let extra = it.next();
            let interp_name = interp.trim_start_matches('/');
            let basename = interp_name.rsplit('/').next().unwrap_or(interp_name);
            // argv = [解释器, 可选解释器参数(缺省用 basename), 脚本, ...原参数]
            let (prog, interp_arg, interp_data) = if let Some(d) = crate::fs::read_file_all(interp_name) {
                (alloc::string::String::from(interp_name), extra.map(|s| alloc::string::String::from(s)), d)
            } else if crate::fs::exists("busybox") {
                // 回退到 busybox 时用 sh 解释（#!/bin/bash 也能跑）
                let arg = match extra {
                    Some(s) => alloc::string::String::from(s),
                    None => alloc::string::String::from("sh"),
                };
                match crate::fs::read_file_all("busybox") {
                    Some(d) => (alloc::string::String::from("busybox"), Some(arg), d),
                    None => return -1,
                }
            } else {
                return -1;
            };
            let mut new_args: Vec<String> = Vec::new();
            new_args.push(prog);
            if let Some(a) = interp_arg {
                new_args.push(a);
            }
            new_args.push(raw_path.clone());
            // Linux 的 binfmt_script：新 argv = [解释器, 可选参数, 脚本路径, 原 argv[1..]]
            for a in args_vec.iter().skip(1) {
                new_args.push(a.clone());
            }
            process.exec(interp_data.as_slice(), new_args);
            return 0;
        }
        // 不是 ELF 也不是脚本：返回 ENOEXEC，避免内核 panic
        if !all_data.starts_with(b"\x7fELF") {
            return -1;
        }
        let argc = args_vec.len();
        process.exec(all_data.as_slice(), args_vec);
        // return argc because cx.x[10] will be covered with it later
        argc as isize
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = current_process();
    loop {
        let mut inner = process.inner_exclusive_access();
        if !inner
            .children
            .iter()
            .any(|p| pid == -1 || pid as usize == p.getpid())
        {
            return -1;
        }
        let pair = inner.children.iter().enumerate().find(|(_, p)| {
            p.inner_exclusive_access().is_zombie && (pid == -1 || pid as usize == p.getpid())
        });
        if let Some((idx, _)) = pair {
            let child = inner.children.remove(idx);
            let found_pid = child.getpid();
            let exit_code = child.inner_exclusive_access().exit_code;
            // Linux 的 wait status 编码：退出码放在高 8 位
            *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code << 8;
            return found_pid as isize;
        }
        drop(inner);
        // 父进程在内核里忙等，时钟中断不会到来，
        // 必须主动轮询定时器，否则子进程的 sleep 永远醒不了。
        crate::timer::check_timer();
        suspend_current_and_run_next();
    }
}

pub fn sys_getppid() -> isize {
    let process = current_process();
    let inner = process.inner_exclusive_access();
    match &inner.parent {
        Some(p) => p.upgrade().map(|p| p.getpid() as isize).unwrap_or(0),
        None => 0,
    }
}

/// uname(2)：填 6 个 65 字节字段
pub fn sys_uname(buf: *mut u8) -> isize {
    let token = current_user_token();
    let fields: [&str; 6] = [
        "Linux",
        "rcore",
        "6.0.0-rcore",
        "#1 rCore-Tutorial",
        "riscv64",
        "localdomain",
    ];
    for (i, f) in fields.iter().enumerate() {
        for j in 0..65usize {
            let c = if j < f.len() { f.as_bytes()[j] } else { 0 };
            *translated_refmut(token, (buf as usize + i * 65 + j) as *mut u8) = c;
        }
    }
    0
}

/// times(2)：返回 100Hz 的时钟滴答数
pub fn sys_times(buf: *mut u8) -> isize {
    let token = current_user_token();
    for i in 0..32usize {
        *translated_refmut(token, (buf as usize + i) as *mut u8) = 0;
    }
    (crate::timer::get_time_ms() / 10) as isize
}

pub fn sys_kill(pid: usize, signal: u32) -> isize {
    if let Some(process) = pid2process(pid) {
        if let Some(flag) = SignalFlags::from_bits(signal) {
            process.inner_exclusive_access().signals |= flag;
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

pub fn sys_exit_group(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

/// 单线程进程，tid 用 pid 代替
pub fn sys_set_tid_address(_tidptr: usize) -> isize {
    current_process().getpid() as isize
}

/// gettimeofday(169)：填 struct timeval { long sec; long usec; }，返回 0
pub fn sys_gettimeofday(tv: *mut u8) -> isize {
    let token = current_user_token();
    let ms = crate::timer::get_time_ms();
    *translated_refmut(token, tv as *mut usize) = ms / 1000;
    *translated_refmut(token, (tv as usize + 8) as *mut usize) = (ms % 1000) * 1000;
    0
}

/// brk(214)：调整 program break，向上增长时按页建立映射
pub fn sys_brk(addr: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    let old = inner.brk;
    if addr == 0 {
        return old as isize;
    }
    if addr < inner.brk_base {
        return old as isize;
    }
    if addr > old {
        let page = crate::config::PAGE_SIZE;
        let old_page = (old + page - 1) / page * page;
        let new_page = (addr + page - 1) / page * page;
        if new_page <= old_page {
            inner.brk = addr;
            return addr as isize;
        }
        let start: crate::mm::VirtAddr = old_page.into();
        let end: crate::mm::VirtAddr = new_page.into();
        inner.memory_set.insert_framed_area(
            start,
            end,
            crate::mm::MapPermission::U | crate::mm::MapPermission::R | crate::mm::MapPermission::W,
        );
    }
    inner.brk = addr;
    addr as isize
}

/// mmap(222)：只支持匿名映射，从固定的 mmap 区向上分配
pub fn sys_mmap(len: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    let size = (len + crate::config::PAGE_SIZE - 1) / crate::config::PAGE_SIZE * crate::config::PAGE_SIZE;
    let start = inner.mmap_top;
    let end = start + size;
    inner.memory_set.insert_framed_area(
        start.into(),
        end.into(),
        crate::mm::MapPermission::U | crate::mm::MapPermission::R | crate::mm::MapPermission::W,
    );
    inner.mmap_top = end;
    start as isize
}

/// munmap(215)：暂不真正回收
pub fn sys_munmap() -> isize {
    0
}
