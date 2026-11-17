use crate::mm::{MapPermission, MemorySet, VirtAddr};
use crate::task::{current_process, current_user_token};
use crate::trap::TrapContext;
use crate::mm::{translated_ref, translated_refmut};

/// 可执行的信号返回 stub 页（li a7,139; ecall; j .）
pub const SIGSTUB_VA: usize = 0x00FF_E000;
/// 保存被打断的 trap context 的页
pub const SIGCTX_VA: usize = 0x00FF_F000;
/// 信号编号：SIGALRM
pub const SIGALRM: usize = 14;

/// 在用户地址空间建立信号所需的两页（exec 时调用）
pub fn setup_signal_pages(memory_set: &mut MemorySet) {
    if memory_set.translate(SIGSTUB_VA.into()).is_none() {
        // addi a7, x0, 139 ; ecall ; j .
        let code: [u8; 12] = [0x93, 0x08, 0xB0, 0x08, 0x73, 0x00, 0x00, 0x00, 0x6F, 0x00, 0x00, 0x00];
        memory_set.insert_data_page(
            VirtAddr::from(SIGSTUB_VA),
            MapPermission::U | MapPermission::R | MapPermission::X,
            &code,
        );
    }
    if memory_set.translate(SIGCTX_VA.into()).is_none() {
        memory_set.insert_data_page(
            VirtAddr::from(SIGCTX_VA),
            MapPermission::U | MapPermission::R | MapPermission::W,
            &[],
        );
    }
}

/// setitimer(103)：只关心 ITIMER_REAL 的 it_value
pub fn sys_setitimer(which: usize, new: *const u8, _old: *mut u8) -> isize {
    if which != 0 || new.is_null() {
        return 0;
    }
    let token = current_user_token();
    // struct itimerval { it_interval{sec,usec}, it_value{sec,usec} }：取 it_value
    let sec = *translated_ref(token, (new as usize + 16) as *const u64) as usize;
    let usec = *translated_ref(token, (new as usize + 24) as *const u64) as usize;
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if sec == 0 && usec == 0 {
        inner.alarm_deadline = 0;
    } else {
        inner.alarm_deadline = crate::timer::get_time_ms() + sec * 1000 + usec / 1000;
    }
    0
}

/// rt_sigaction(134)：记录 SIGALRM 的 handler
pub fn sys_rt_sigaction(signo: usize, act: *const u8, _old: *mut u8) -> isize {
    if signo != SIGALRM {
        return 0;
    }
    if act.is_null() {
        return 0;
    }
    let token = current_user_token();
    let handler = *translated_ref(token, act as *const usize);
    let process = current_process();
    process.inner_exclusive_access().sigalrm_handler = handler;
    0
}

/// rt_sigreturn(139)：恢复被打断的上下文
pub fn sys_rt_sigreturn() -> isize {
    let token = current_user_token();
    let cx = crate::task::current_trap_cx();
    let dst = cx as *mut TrapContext as *mut u8;
    for i in 0..core::mem::size_of::<TrapContext>() {
        let b = *translated_ref(token, (SIGCTX_VA + i) as *const u8);
        unsafe { *dst.add(i) = b; }
    }
    let process = current_process();
    process.inner_exclusive_access().in_sigalrm = false;
    0
}

/// 定时器中断里调用：到期则标记 SIGALRM 待投递
pub fn check_alarm() {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if inner.alarm_deadline != 0 && crate::timer::get_time_ms() >= inner.alarm_deadline {
        inner.alarm_deadline = 0;
        inner.sigalrm_pending = true;
    }
}

/// 内核忙等循环（waitpid / futex …）里调用：SIGALRM 已到期则返回 true。
/// 调用者应立刻返回 -EINTR，这样回到用户态后 deliver_signal 才能执行超时 handler。
/// （LTP 的超时杀子进程、busybox timeout 都依赖这个）
pub fn alarm_interrupts() -> bool {
    check_alarm();
    current_process().inner_exclusive_access().sigalrm_pending
}

/// 返回用户态前调用：若有待投递的 SIGALRM，改成跳到 handler
pub fn deliver_signal() {
    let process = current_process();
    let (handler, pending, in_handler) = {
        let inner = process.inner_exclusive_access();
        (inner.sigalrm_handler, inner.sigalrm_pending, inner.in_sigalrm)
    };
    if !pending || handler == 0 || handler == 1 || in_handler {
        let mut inner = process.inner_exclusive_access();
        inner.sigalrm_pending = false;
        return;
    }
    let token = current_user_token();
    let cx = crate::task::current_trap_cx();
    // 保存当前上下文
    let saved = *cx;
    let bytes = unsafe {
        core::slice::from_raw_parts(&saved as *const TrapContext as *const u8, core::mem::size_of::<TrapContext>())
    };
    for (i, b) in bytes.iter().enumerate() {
        *translated_refmut(token, (SIGCTX_VA + i) as *mut u8) = *b;
    }
    // 构造调用 handler 的上下文
    cx.sepc = handler;
    cx.x[1] = SIGSTUB_VA;
    cx.x[10] = SIGALRM;
    cx.x[2] = cx.x[2].saturating_sub(512) & !0xf;
    let mut inner = process.inner_exclusive_access();
    inner.sigalrm_pending = false;
    inner.in_sigalrm = true;
}
