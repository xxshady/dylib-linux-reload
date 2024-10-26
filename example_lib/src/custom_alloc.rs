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

        if ALLOC_INIT.load(Ordering::SeqCst) {
            crate::ON_ALLOC(
                ptr,
                c_layout,
            );
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

        save_dealloc_in_buffer(ptr, c_layout);
    }
}

const ALLOCS_CACHE_SIZE: usize = 20_000;
const TRANSPORT_BUFFER_SIZE: usize = 20_000;

type AllocsCache = HashMap<*mut u8, AllocatorOp>;

struct AllocsCacheContainer(HashMap<*mut u8, AllocatorOp>);

unsafe impl Send for AllocsCacheContainer {}
unsafe impl Sync for AllocsCacheContainer {}

static ALLOCS_CACHE: LazyLock<Mutex<AllocsCacheContainer>> = LazyLock::new(|| Mutex::new(AllocsCacheContainer(HashMap::new())));
static ALLOC_INIT: AtomicBool = AtomicBool::new(false);

static TRANSPORT_BUFFER: Mutex<Vec<AllocatorOp>> = Mutex::new(Vec::new());

fn lock_allocs_cache() -> MutexGuard<'static, AllocsCacheContainer> {
    ALLOCS_CACHE.try_lock().unwrap_or_else(|_|{
        unsafe {
            crate::PRINT("fatal error: failed to lock ALLOCS_CACHE");
        }
        std::process::abort();
    })
}

fn lock_transport_buffer() -> MutexGuard<'static, Vec<AllocatorOp>> {
    TRANSPORT_BUFFER.try_lock().unwrap_or_else(|_| {
        unsafe {
            crate::PRINT("fatal error: failed to lock TRANSPORT_BUFFER");
        }
        std::process::abort();
    })
}

fn push_to_allocs_cache(op: AllocatorOp, cache: Option<&mut AllocsCache>) {
    let cache = if let Some(cache) = cache {
        cache
    } else {
        &mut lock_allocs_cache().0
    };

    let ptr = match op {
        AllocatorOp::Alloc(Allocation(ptr, ..)) => {
            ptr
        }
        AllocatorOp::Dealloc(Allocation(ptr, ..)) => {
            ptr
        }
    };

    if cache.contains_key(&ptr) {
        unsafe {
            crate::PRINT("fatal error: cannot push ptr duplicate to ALLOCS_CACHE");
        }
        std::process::abort();
    }

    cache.insert(ptr, op);

    if cache.len() == ALLOCS_CACHE_SIZE {
        send_cached_allocs(Some(cache));
    }
}

fn save_alloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    // unsafe { crate::PRINT("save_alloc_in_buffer"); }

    push_to_allocs_cache(AllocatorOp::Alloc(Allocation(ptr, layout)), None);
}

fn save_dealloc_in_buffer(ptr: *mut u8, layout: CLayout) {
    // unsafe { crate::PRINT("save_dealloc_in_buffer"); }

    let mut cache = &mut lock_allocs_cache().0;

    // if cache did not contain this allocation only host knows about it
    // so we need to send it immediately
    // we can't put it into cache because of such scenario:
    // 1. host contains ptr A with size = 1
    // 2. guest deallocates ptr A (puts it into cache)
    // 3. guest allocates new memory in ptr A with size = 2 (puts it into cache)
    // 4. cache gets sent to host
    // 5. host contains two ptr A
    // TODO: so host should also use hashmap and save_dealloc_in_buffer should be similar to save_alloc_in_buffer:
    // insert AllocatorOp::Dealloc (which will replace alloc automatically) 
    // and check for len to prevent allocations
    if cache.remove(&ptr).is_none() {
        unsafe {
            crate::ON_DEALLOC(ptr, layout);
        }
    }
}

fn allocation_not_found() -> ! {
    // TODO: improve error message but be careful about allocations!!!
    unsafe {
        crate::PRINT("fatal error: unknown allocation");
    }
    std::process::abort();
}

// TODO: get rid of ALLOC_INIT 
pub unsafe fn init() {
    ALLOC_INIT.swap(true, Ordering::SeqCst);

    let mut cache = &mut lock_allocs_cache().0;
    cache.reserve(ALLOCS_CACHE_SIZE);

    let mut transport = lock_transport_buffer();
    transport.reserve(TRANSPORT_BUFFER_SIZE);

    ALLOC_INIT.swap(false, Ordering::SeqCst);
}

pub fn send_cached_allocs(cache: Option<&mut AllocsCache>) {
    let cache = if let Some(cache) = cache {
        cache
    } else {
        &mut lock_allocs_cache().0
    };

    let mut transport = lock_transport_buffer();

    let free_space = transport.capacity() - transport.len();
    if free_space < cache.len() {
        unsafe {
            crate::PRINT("fatal error: TRANSPORT_BUFFER won't be able to hold all ops from ALLOCS_CACHE");
        }
        std::process::abort();
    }

    transport.extend(cache.drain().map(|(address, allocation)| allocation));
    unsafe {
        crate::SEND_CACHED_ALLOCS(&transport);
    }
    transport.clear();
}
