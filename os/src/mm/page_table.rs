use super::{frame_alloc, FrameTracker, PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

bitflags! {
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// 内核代用户访问内存前，确保目标虚拟页已经有有效 PTE。
/// brk/mmap 是惰性映射：内核直接读写用户缓冲区（read/write/pipe/stat ...）时，
/// 这些页可能还没被用户态碰过、页表里也没有 PTE，这里补一次缺页。
/// 返回 false 表示不是当前进程可补页的合法地址。
/// 非法用户地址的落点：一块内核哑页。写进去的数据被丢弃，读出来是 0。
/// 目的：系统调用拿到空指针/越界指针时不要 panic 掉整个内核（LTP 会大量这么干）。
static mut DUMMY_USER_PAGE: [u8; 4096] = [0u8; 4096];

fn dummy_user_page() -> *mut u8 {
    unsafe { core::ptr::addr_of_mut!(DUMMY_USER_PAGE) as *mut u8 }
}

fn ensure_user_page_mapped(token: usize, va: usize) -> bool {
    let vpn = VirtAddr::from(va).floor();
    // 快路径：已经有有效 PTE 就不需要动进程锁
    let mapped = PageTable::from_token(token)
        .translate(vpn)
        .map(|pte| pte.is_valid())
        .unwrap_or(false);
    if mapped {
        return true;
    }
    if let Some(task) = crate::task::current_task() {
        if let Some(process) = task.process.upgrade() {
            if let Some(mut inner) = process.try_inner_exclusive_access() {
                if inner.memory_set.token() == token {
                    return inner.memory_set.handle_page_fault(vpn);
                }
            }
        }
    }
    false
}

pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        let page_end = usize::from(end_va);
        if !ensure_user_page_mapped(token, start) {
            // 非法用户缓冲区：退回哑页（读 0 / 写丢弃），不让内核 panic
            let n = if page_end > start { page_end - start } else { 0 };
            if n > 0 {
                v.push(unsafe { core::slice::from_raw_parts_mut(dummy_user_page(), n) });
            }
            start = page_end;
            continue;
        }
        let ppn = page_table.translate(start_va.floor()).unwrap().ppn();
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = page_end;
    }
    v
}

/// Load a string from other address spaces into kernel space without an end `\0`.
pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    let mut checked_vpn = usize::MAX;
    loop {
        let vpn = VirtAddr::from(va).floor().0;
        if vpn != checked_vpn {
            if !ensure_user_page_mapped(token, va) {
                // 非法指针：按空串处理，避免内核 panic
                return string;
            }
            checked_vpn = vpn;
        }
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .get_mut());
        if ch == 0 {
            break;
        }
        string.push(ch as char);
        va += 1;
        // 防御：用户给的“字符串”可能没有结尾 0
        if string.len() > 4095 {
            break;
        }
    }
    string
}

pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let va = ptr as usize;
    if !ensure_user_page_mapped(token, va) {
        return unsafe { &*(dummy_user_page() as *const T) };
    }
    let page_table = PageTable::from_token(token);
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_ref()
}

pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let va = ptr as usize;
    if !ensure_user_page_mapped(token, va) {
        return unsafe { &mut *(dummy_user_page() as *mut T) };
    }
    let page_table = PageTable::from_token(token);
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_mut()
}

pub struct UserBuffer {
    pub buffers: Vec<&'static mut [u8]>,
}

impl UserBuffer {
    pub fn new(buffers: Vec<&'static mut [u8]>) -> Self {
        Self { buffers }
    }
    pub fn len(&self) -> usize {
        let mut total: usize = 0;
        for b in self.buffers.iter() {
            total += b.len();
        }
        total
    }
}

impl IntoIterator for UserBuffer {
    type Item = *mut u8;
    type IntoIter = UserBufferIterator;
    fn into_iter(self) -> Self::IntoIter {
        UserBufferIterator {
            buffers: self.buffers,
            current_buffer: 0,
            current_idx: 0,
        }
    }
}

pub struct UserBufferIterator {
    buffers: Vec<&'static mut [u8]>,
    current_buffer: usize,
    current_idx: usize,
}

impl Iterator for UserBufferIterator {
    type Item = *mut u8;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_buffer >= self.buffers.len() {
            None
        } else {
            let r = &mut self.buffers[self.current_buffer][self.current_idx] as *mut _;
            if self.current_idx + 1 == self.buffers[self.current_buffer].len() {
                self.current_idx = 0;
                self.current_buffer += 1;
            } else {
                self.current_idx += 1;
            }
            Some(r)
        }
    }
}
