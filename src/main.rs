use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
        MutexGuard,
        LazyLock,
    },
    thread::ThreadId,
    collections::HashMap,
};

use std::ffi::c_void;

include!("../shared/lib.rs");
use shared::{Allocation, CLayout, AllocatorOp, AllocatorPtr};

// TODO: is it needed here?
// #[global_allocator]
// static GLOBAL: System = System;

fn main() {
    for _ in 1..=3 {
        load_and_unload();
        println!("----------------------------");
        // std::thread::sleep_ms(1000);
    }
}

fn load_and_unload() {
    unsafe {
        // I could use `std::thread::current().id()`
        // but I'm not sure how safe it is for FFI (+ it needs to be stored in a static)
        // since it's an opaque object and as_u64() is unstable
        let main_thread_id = libc::syscall(libc::SYS_gettid);

        let directory = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        // this flag allows us to replace __cxa_thread_atexit_impl in dynamic library
        const RTLD_DEEPBIND: i32 = 0x00008;
        let lib = libloading::os::unix::Library::open(
            Some(format!("target/{directory}/libexample_lib.so")),
            RTLD_LAZY | RTLD_LOCAL | RTLD_DEEPBIND,
        )
        .unwrap();

        static ALLOCS: LazyLock<Mutex<HashMap<AllocatorPtr, Allocation>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

        fn lock_allocs() -> MutexGuard<'static, HashMap<AllocatorPtr, Allocation>> {
            let Ok(allocs) = ALLOCS.lock() else {
                eprintln!("failed to lock ALLOCS");
                std::process::abort();
            };
            
            allocs
        }

        let on_alloc_static: *mut unsafe extern "C" fn(*mut u8, CLayout) =
            *lib.get(b"ON_ALLOC\0").unwrap();
        *on_alloc_static = on_alloc;

        unsafe extern "C" fn on_alloc(ptr: *mut u8, layout: CLayout) {
            let thread_id = std::thread::current().id();

            // println!("alloc {ptr:?} {thread_id:?}");

            let mut allocs = lock_allocs();
            let ptr = AllocatorPtr(ptr);
            allocs.insert(ptr, Allocation(ptr, layout));
        }

        let on_dealloc_static: *mut unsafe extern "C" fn(*mut u8, CLayout) =
            *lib.get(b"ON_DEALLOC\0").unwrap();
        *on_dealloc_static = on_dealloc;

        unsafe extern "C" fn on_dealloc(ptr: *mut u8, layout: CLayout) {
            // println!("dealloc {ptr:?}");

            let mut allocs = lock_allocs();
            allocs.remove(&AllocatorPtr(ptr)).unwrap_or_else(|| {
                eprintln!("did not found allocation: {ptr:?}");
                std::process::abort();
            });
        }

        let send_cached_allocs_static: *mut unsafe extern "C" fn(&[AllocatorOp]) =
        *lib.get(b"SEND_CACHED_ALLOCS\0").unwrap();
        *send_cached_allocs_static = send_cached_allocs;

        unsafe extern "C" fn send_cached_allocs(ops: &[AllocatorOp]) {
            println!("received cached alloc ops: {}", ops.len());
            // println!("{ops:?}");

            let mut allocs = lock_allocs();
            for op in ops {
                match op {
                    AllocatorOp::Alloc(allocation) => {
                        let Allocation(ptr, ..) = allocation;
                        allocs.insert(*ptr, allocation.clone());
                    }
                    AllocatorOp::Dealloc(Allocation(ptr, ..)) => {
                        // doesnt matter if allocs didnt have it
                        let _ = allocs.remove(ptr);
                    }
                }
            }
        }

        let resource_main_thread_id = std::thread::current().id();

        let exit_deallocation: *mut bool = *lib.get(b"EXIT_DEALLOCATION\0").unwrap();
        if *exit_deallocation {
            panic!(
                "library must be unloaded before calling main \n{}",
                "note: before unloading the library, make sure that all threads are joined (if any were spawned by it)"
            );
        }

        let print: *mut unsafe extern "C" fn(&str) = *lib.get(b"PRINT\0").unwrap();
        *print = print_impl;

        type MainFn =
            unsafe extern "C" fn(main_resoure_thread_id: i64);

        let main_fn: MainFn = *lib.get(b"main\0").unwrap();

        // let catch_undwind = std::panic::catch_unwind(|| {
        main_fn(main_thread_id);
        // });
        // println!("main fn catch_undwind: {catch_undwind:?}");

        unsafe extern "C" fn print_impl(message: &str) {
            if message.starts_with("fatal error:") {
                let backtrace = std::backtrace::Backtrace::force_capture();
                println!("backtrace: {backtrace}");
            }
            println!("dylib: {message}");
        }

        type CallThreadLocalDestructorsFn = unsafe extern "C" fn();

        let call_destructors: CallThreadLocalDestructorsFn =
            *lib.get(b"run_thread_local_dtors\0").unwrap();
        call_destructors();

        println!("requesting remaining alloc ops");

        let request_cached_allocs: unsafe extern "C" fn() =
            *lib.get(b"request_cached_allocs\0").unwrap();

        request_cached_allocs();

        let mut allocs = lock_allocs();
        println!("deallocating remaining memory ({})", allocs.len());

        let exit_fn: unsafe extern "C" fn(&[Allocation]) = *lib.get(b"exit\0").unwrap();

        // TEST
        {
            let allocs = std::mem::take(&mut *allocs);
            let allocs: Box<[Allocation]> = allocs.into_iter().map(|(_, allocation)| allocation).collect();
            exit_fn(&allocs);
        }
        drop(allocs);

        // TODO: add detection of detached threads (probably other stuff) which prevents library from unloading
        // by trying to load that library again and checking static var
        // libloading crate will call dlclose in Drop implementation for us
        // (explicit drop call for clarity)
        // drop(lib);
        lib.close().unwrap();
    }
}
