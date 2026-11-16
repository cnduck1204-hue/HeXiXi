#![no_std]
#![no_main]

//! 开机自动执行 basic 组全部用例，输出官方评测要求的标记，最后正常退出（触发关机）。
//! 仅在测试盘里存在 run_tests 标记文件时被内核当作 initproc。

#[macro_use]
extern crate user_lib;

use user_lib::{exec, exit, fork, waitpid};

static TESTS: &[&[u8]] = &[
    b"t_getpid\0",
    b"t_getppid\0",
    b"t_uname\0",
    b"t_times\0",
    b"t_sleep\0",
    b"t_exit\0",
    b"t_fork\0",
    b"t_wait\0",
    b"t_waitpid\0",
    b"t_open\0",
    b"t_read\0",
    b"t_write\0",
    b"t_close\0",
    b"t_dup\0",
    b"t_dup2\0",
    b"t_openat\0",
    b"t_chdir\0",
    b"t_getcwd\0",
    b"t_fstat\0",
    b"t_mkdir_\0",
    b"t_unlink\0",
    b"t_mount\0",
    b"t_umount\0",
    b"t_getdents\0",
    b"t_execve\0",
    b"t_pipe\0",
];

#[no_mangle]
pub fn main() -> i32 {
    println!("#### OS COMP TEST GROUP START basic ####");
    for t in TESTS.iter() {
        let name = core::str::from_utf8(&t[..t.len() - 1]).unwrap();
        let pid = fork();
        if pid == 0 {
            // 内核按 NULL 结尾的指针数组解析 argv，必须传真实数组
            let null_argv: [*const u8; 1] = [core::ptr::null()];
            exec(name, &null_argv);
            // exec 失败才会走到这里
            println!("  [autorun] exec {} failed", name);
            exit(-1);
        } else if pid > 0 {
            let mut code = 0;
            waitpid(pid as usize, &mut code);
        } else {
            println!("  [autorun] fork failed");
        }
    }
    println!("#### OS COMP TEST GROUP END basic ####");
    0
}
