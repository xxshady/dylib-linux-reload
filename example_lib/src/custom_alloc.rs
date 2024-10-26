use std::{
    alloc::{GlobalAlloc, Layout, System},
    ops,
    sync::{Mutex, MutexGuard, LazyLock, atomic::{AtomicIsize, AtomicUsize, AtomicBool, Ordering}},
    collections::HashMap,
};

use crate::shared::{Allocation, CLayout, AllocatorOp};

#[derive(Default, Debug)]
pub struct CustomAlloc {
    inner: System,
}

impl CustomAlloc {
    pub const fn new() -> Self {
        CustomAlloc { inner: System }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc(layout);

        let c_layout = crate::shared::CLayout {
            size: layout.size(),
            align: layout.align(),
        };

        if ALLOCS_BUFFER_INIT.load(Ordering::SeqCst) {
            crate::ON_ALLOC(
                ptr,
                c_layout,
            )
        } else {
            save_alloc_in_buffer(ptr, c_layout);
        }

        // if !crate::CAPTURING_BACKTRACE.load(Ordering::SeqCst) {
            // crate::ON_ALLOC(
            //     ptr,
            //     c_layout,
            // );
        // } else {
        //     save_alloc_in_buffer(ptr, c_layout);
        // }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.dealloc(ptr, layout);
        
        if crate::EXIT_DEALLOCATION {
            return;
        }

        let c_layout = crate::shared::CLayout {
            size: layout.size(),
            align: layout.align(),
        };

        save_dealloc_in_buffer(ptr, c_layout);

        // if !crate::CAPTURING_BACKTRACE.load(Ordering::SeqCst) {
            // crate::ON_DEALLOC(
            //     ptr,
            //     c_layout,
            // );
        // } else {
        //     save_dealloc_in_buffer(ptr, c_layout);
        // }
    }
}

const ALLOCS_BUFFER_SIZE: usize = 20_000;

// TODO: is it safe to convert *mut u8 to usize?
type AllocsCache = HashMap<usize, AllocatorOp>;
static ALLOCS_BUFFER: LazyLock<Mutex<AllocsCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static ALLOCS_BUFFER_INIT: AtomicBool = AtomicBool::new(false);
static mut ALLOCS_BUFFER_EMPTY: bool = true;

fn lock_allocs_buffer() -> MutexGuard<'static, AllocsCache> {
    let Ok(allocs) = ALLOCS_BUFFER.try_lock() else {
        unsafe {
            crate::PRINT("fatal error: failed to lock ALLOCS_BUFFER");
        }
        std::process::abort();
    };
    allocs
}

fn push_to_allocs_buffer(op: AllocatorOp, allocs: Option<&mut AllocsCache>) {
    unsafe {
        ALLOCS_BUFFER_EMPTY = false;
    }

    let mut allocs = if let Some(allocs) = allocs {
        allocs
    } else {
        &mut lock_allocs_buffer()
    };
    
    if allocs.len() == ALLOCS_BUFFER_SIZE {
        send_bulk_allocs(Some(allocs));    
    }

    let address = match op {
        AllocatorOp::Alloc(Allocation(ptr, ..)) => {
            ptr
        }
        AllocatorOp::Dealloc(Allocation(ptr, ..)) => {
            ptr
        }
    };

    allocs.insert(address as usize, op);
}

fn save_alloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    unsafe { crate::PRINT("save_alloc_in_buffer"); }
    push_to_allocs_buffer(AllocatorOp::Alloc(Allocation(ptr, layout)), None);
}

fn save_dealloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    unsafe { crate::PRINT("save_dealloc_in_buffer"); }

    let mut allocs = lock_allocs_buffer();

    if allocs.remove(&(ptr as usize)).is_none() {
        push_to_allocs_buffer(AllocatorOp::Dealloc(Allocation(ptr, layout)), Some(&mut allocs));
    }
}

fn allocation_not_found() -> ! {
    // TODO: improve error message but be careful about allocations!!!
    unsafe {
        crate::PRINT("unknown allocation");
    }
    std::process::abort();
}

// TODO: get rid of ALLOCS_BUFFER_INIT 
pub unsafe fn init() {
    ALLOCS_BUFFER_INIT.swap(true, Ordering::SeqCst);
    let mut allocs = lock_allocs_buffer();
    allocs.reserve(ALLOCS_BUFFER_SIZE);
    ALLOCS_BUFFER_INIT.swap(false, Ordering::SeqCst);
}

pub fn send_bulk_allocs(allocs: Option<&mut AllocsCache>) {
    let mut allocs = if let Some(allocs) = allocs {
        allocs
    } else {
        &mut lock_allocs_buffer()
    };
    unsafe {
        let values: Vec<AllocatorOp> = allocs.values().cloned().collect();
        crate::BULK_ALLOCATIONS(&values);
    }

    allocs.clear();
    unsafe {
        ALLOCS_BUFFER_EMPTY = true;
    }
}
