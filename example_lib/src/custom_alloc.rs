use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::HashMap,
    ops,
    sync::{
        atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering},
        LazyLock, Mutex, MutexGuard,
    },
};

use crate::shared::{Allocation, AllocatorOp, AllocatorPtr, CLayout};

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

        if ALLOC_INIT.load(Ordering::SeqCst) {
            crate::ON_ALLOC(ptr, c_layout);
        } else {
            save_alloc_in_buffer(ptr, c_layout);
        }

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

        if ALLOC_UNLOAD.load(Ordering::SeqCst) {
            crate::ON_DEALLOC(ptr, c_layout);
            return;
        }

        save_dealloc_in_buffer(ptr, c_layout);
    }
}

const CACHE_SIZE: usize = 20_000;

type AllocsCache = HashMap<AllocatorPtr, AllocatorOp>;
pub type AllocsCacheGuard = InitedGuard<'static, AllocsCache>;

static ALLOCS_CACHE: Mutex<Option<AllocsCache>> = Mutex::new(None);
static ALLOC_INIT: AtomicBool = AtomicBool::new(false);
static ALLOC_UNLOAD: AtomicBool = AtomicBool::new(false);

static TRANSPORT_BUFFER: Mutex<Vec<AllocatorOp>> = Mutex::new(Vec::new());

pub struct InitedGuard<'a, T>(MutexGuard<'a, Option<T>>);

impl<T> InitedGuard<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.0.as_mut().unwrap_or_else(|| {
            unsafe {
                crate::PRINT("fatal error: InitedGuard mutex must be initialized");
            }
            std::process::abort();
        })
    }
}

fn lock_allocs_cache() -> AllocsCacheGuard {
    let guard = ALLOCS_CACHE.lock().unwrap_or_else(|_| {
        unsafe {
            crate::PRINT("fatal error: failed to lock ALLOCS_CACHE");
        }
        std::process::abort();
    });

    InitedGuard(guard)
}

fn lock_transport_buffer() -> MutexGuard<'static, Vec<AllocatorOp>> {
    TRANSPORT_BUFFER.lock().unwrap_or_else(|_| {
        unsafe {
            crate::PRINT("fatal error: failed to lock TRANSPORT_BUFFER");
        }
        std::process::abort();
    })
}

fn push_to_allocs_cache(op: AllocatorOp, cache: Option<AllocsCacheGuard>) {
    let mut cache_guard = if let Some(cache) = cache {
        cache
    } else {
        lock_allocs_cache()
    };
    let cache = cache_guard.as_mut();

    let ptr = match op {
        AllocatorOp::Alloc(Allocation(ptr, ..)) => ptr,
        AllocatorOp::Dealloc(Allocation(ptr, ..)) => ptr,
    };

    cache.insert(ptr, op);

    if cache.len() == CACHE_SIZE {
        send_cached_allocs(Some(cache_guard));
    }
}

fn save_alloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    // unsafe { crate::PRINT("save_alloc_in_buffer"); }

    push_to_allocs_cache(
        AllocatorOp::Alloc(Allocation(AllocatorPtr(ptr), layout)),
        None,
    );
}

fn save_dealloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    // unsafe { crate::PRINT("save_dealloc_in_buffer"); }

    let ptr = AllocatorPtr(ptr);
    push_to_allocs_cache(AllocatorOp::Dealloc(Allocation(ptr, layout)), None);
}

fn allocation_not_found() -> ! {
    // TODO: improve error message but be careful about allocations!!!
    unsafe {
        crate::PRINT("fatal error: unknown allocation");
    }
    std::process::abort();
}

pub unsafe fn init() {
    ALLOC_INIT.swap(true, Ordering::SeqCst);

    let mut cache = ALLOCS_CACHE.try_lock().unwrap_or_else(|_| {
        unsafe {
            crate::PRINT("fatal error: failed to lock ALLOCS_CACHE for initialization");
        }
        std::process::abort();
    });
    cache.replace(HashMap::with_capacity(CACHE_SIZE));

    let mut transport = lock_transport_buffer();
    transport.reserve(CACHE_SIZE);

    ALLOC_INIT.swap(false, Ordering::SeqCst);
}

pub fn send_cached_allocs(cache: Option<AllocsCacheGuard>) {
    let mut cache = if let Some(cache) = cache {
        cache
    } else {
        lock_allocs_cache()
    };
    let cache = cache.as_mut();

    let mut transport = lock_transport_buffer();

    // TODO: remove it? since we have shared CACHE_SIZE constant
    // let free_space = transport.capacity() - transport.len();
    // if free_space < cache.len() {
    //     unsafe {
    //         crate::PRINT("fatal error: TRANSPORT_BUFFER won't be able to hold all ops from ALLOCS_CACHE");
    //     }
    //     std::process::abort();
    // }

    transport.extend(cache.drain().map(|(_, allocation)| allocation));
    unsafe {
        crate::SEND_CACHED_ALLOCS(&transport);
    }
    transport.clear();
}

pub fn unload() {
    ALLOC_UNLOAD.swap(true, Ordering::SeqCst);

    let mut cache = ALLOCS_CACHE.try_lock().unwrap_or_else(|_| {
        unsafe {
            crate::PRINT("fatal error: failed to lock ALLOCS_CACHE for unload");
        }
        std::process::abort();
    });
    cache.take().unwrap_or_else(|| {
        unsafe {
            crate::PRINT("fatal error: failed to unload ALLOCS_CACHE");
        }
        std::process::abort();
    });

    ALLOC_UNLOAD.swap(false, Ordering::SeqCst);
}
