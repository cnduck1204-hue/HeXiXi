use super::id::RecycleAllocator;
use super::manager::insert_into_pid2process;
use super::TaskControlBlock;
use super::{add_task, SignalFlags};
use super::{pid_alloc, PidHandle};
use crate::fs::{File, Stdin, Stdout};
use crate::mm::{translated_refmut, MemorySet, KERNEL_SPACE};
use crate::sync::{Condvar, Mutex, Semaphore, UPSafeCell};
use crate::trap::{trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

pub struct ProcessControlBlock {
    // immutable
    pub pid: PidHandle,
    // mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    pub signals: SignalFlags,
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    pub task_res_allocator: RecycleAllocator,
    pub mutex_list: Vec<Option<Arc<dyn Mutex>>>,
    pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    pub condvar_list: Vec<Option<Arc<Condvar>>>,
    /// 当前工作目录（easy-fs 是扁平命名空间，仅记录字符串）
    pub cwd: String,
    /// 堆起始（可增长下界）
    pub brk_base: usize,
    /// 当前 program break
    pub brk: usize,
    /// mmap 分配起点（向上增长）
    pub mmap_top: usize,
}

impl ProcessControlBlockInner {
    #[allow(unused)]
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        // allocate a pid
        let pid_handle = pid_alloc();
        let process = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                    cwd: String::from("/"),
                    brk_base: 0,
                    brk: 0,
                    mmap_top: 0x1000_0000,
                })
            },
        });
        // create a main thread, we should allocate ustack and trap_cx here
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        // prepare trap_cx of main thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );
        // add main thread to the process
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));
        // add main thread to scheduler
        add_task(task);
        process
    }

    /// Only support processes with a single thread.
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        // 堆放在独立地址段，避免与用户栈/程序段重叠
        let heap_base: usize = 0x2000_0000;
        // substitute memory_set
        self.inner_exclusive_access().memory_set = memory_set;
        // then we alloc user resource for main thread again
        // since memory_set has been changed
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();
        {
            let mut pinner = self.inner_exclusive_access();
            pinner.brk_base = heap_base;
            pinner.brk = heap_base;
        }
        // ---- 构造 Linux 进程启动栈 ----
        // 低地址 -> 高地址：argc | argv[..] | NULL | NULL(envp) | auxv | 字符串/程序头/随机数
        let mut sp = task_inner.res.as_mut().unwrap().ustack_top();
        let argc = args.len();

        // 1. 参数字符串（先放到高地址区）
        let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argc);
        for arg in args.iter().rev() {
            sp -= arg.len() + 1;
            let mut p = sp;
            for c in arg.as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
            arg_ptrs.push(sp);
        }
        arg_ptrs.reverse();

        // 2. 程序头表副本（AT_PHDR 指向它）
        let e_entry = u64::from_le_bytes(elf_data[0x18..0x20].try_into().unwrap()) as usize;
        let e_phoff = u64::from_le_bytes(elf_data[0x20..0x28].try_into().unwrap()) as usize;
        let e_phentsize = u16::from_le_bytes(elf_data[0x36..0x38].try_into().unwrap()) as usize;
        let e_phnum = u16::from_le_bytes(elf_data[0x38..0x3a].try_into().unwrap()) as usize;
        let phdr_len = e_phnum * e_phentsize;
        sp -= phdr_len;
        sp -= sp % 8;
        let phdr_addr = sp;
        for i in 0..phdr_len {
            *translated_refmut(new_token, (phdr_addr + i) as *mut u8) = elf_data[e_phoff + i];
        }

        // 3. AT_RANDOM 的 16 字节
        sp -= 16;
        let random_addr = sp;
        for i in 0..16usize {
            *translated_refmut(new_token, (random_addr + i) as *mut u8) =
                (i as u8).wrapping_mul(37).wrapping_add(11);
        }

        // 4. 数组区：argc / argv / envp / auxv 必须连续
        let auxv: [(usize, usize); 15] = [
            (3, phdr_addr),
            (4, e_phentsize),
            (5, e_phnum),
            (6, 4096),
            (9, e_entry),
            (11, 0),
            (12, 0),
            (13, 0),
            (14, 0),
            (23, 0),
            (25, random_addr),
            (31, arg_ptrs.first().copied().unwrap_or(0)),
            (17, 100),
            (16, 0),
            (0, 0),
        ];
        let words = 1 + argc + 1 + 1 + auxv.len() * 2;
        sp -= words * core::mem::size_of::<usize>();
        sp -= sp % 16;
        let base = sp;
        let sz = core::mem::size_of::<usize>();
        *translated_refmut(new_token, base as *mut usize) = argc;
        for i in 0..argc {
            *translated_refmut(new_token, (base + (1 + i) * sz) as *mut usize) = arg_ptrs[i];
        }
        *translated_refmut(new_token, (base + (1 + argc) * sz) as *mut usize) = 0;
        *translated_refmut(new_token, (base + (2 + argc) * sz) as *mut usize) = 0;
        for (i, (t, v)) in auxv.iter().enumerate() {
            *translated_refmut(new_token, (base + (3 + argc + i * 2) * sz) as *mut usize) = *t;
            *translated_refmut(new_token, (base + (4 + argc + i * 2) * sz) as *mut usize) = *v;
        }
        let user_sp = base;
        // initialize trap_cx
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = user_sp + core::mem::size_of::<usize>();
        *task_inner.get_trap_cx() = trap_cx;
    }

    /// Only support processes with a single thread.
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        // clone parent's memory_set completely including trampoline/ustacks/trap_cxs
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // alloc a pid
        let pid = pid_alloc();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        // create child process pcb
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                    cwd: String::from("/"),
                    brk_base: 0,
                    brk: 0,
                    mmap_top: 0x1000_0000,
                })
            },
        });
        // 子进程继承父进程的工作目录
        child.inner_exclusive_access().cwd = parent.cwd.clone();
        // add child
        parent.children.push(Arc::clone(&child));
        // create main thread of child process
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_base(),
            // here we do not allocate trap_cx or ustack again
            // but mention that we allocate a new kstack here
            false,
        ));
        // attach task to child process
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        drop(child_inner);
        // modify kstack_top in trap_cx of this thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        drop(task_inner);
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        // add this thread to scheduler
        add_task(task);
        child
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}
