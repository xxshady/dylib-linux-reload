use libc::RTLD_DEEPBIND;
use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};
use stabby::{libloading::StabbyLibrary, str::Str};
use std::{
    collections::HashMap,
    io::stdin,
    sync::{LazyLock, Mutex, MutexGuard},
};

use shared::{
    Allocation, AllocatorOp, AllocatorPtr, SliceAllocation, SliceAllocatorOp, StableLayout,
};

// TODO: is it needed here?
// #[global_allocator]
// static GLOBAL: System = System;

fn main() {
    loop {
        load_and_unload();
        println!("----------------------------");

        let mut message = String::new();
        stdin().read_line(&mut message).unwrap();
        if message == "q\n" {
            return;
        }
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

        // RTLD_DEEPBIND allows replacing __cxa_thread_atexit_impl only for dynamic library
        // without replacing it for the whole executable
        let lib = libloading::os::unix::Library::open(
            Some(format!("target/{directory}/libexample_lib.so")),
            RTLD_LAZY | RTLD_LOCAL | RTLD_DEEPBIND,
        )
        .unwrap();
        let lib = libloading::Library::from(lib);

        static ALLOCS: LazyLock<Mutex<HashMap<AllocatorPtr, Allocation>>> =
            LazyLock::new(|| Mutex::new(HashMap::new()));

        fn lock_allocs() -> MutexGuard<'static, HashMap<AllocatorPtr, Allocation>> {
            let Ok(allocs) = ALLOCS.lock() else {
                eprintln!("failed to lock ALLOCS");
                std::process::abort();
            };

            allocs
        }

        let on_alloc_static: *mut extern "C" fn(*mut u8, StableLayout) =
            *lib.get(b"ON_ALLOC\0").unwrap();
        *on_alloc_static = on_alloc;

        extern "C" fn on_alloc(ptr: *mut u8, layout: StableLayout) {
            let mut allocs = lock_allocs();
            let ptr = AllocatorPtr(ptr);
            allocs.insert(ptr, Allocation(ptr, layout));
        }

        let on_dealloc_static: *mut extern "C" fn(*mut u8, StableLayout) =
            *lib.get(b"ON_DEALLOC\0").unwrap();
        *on_dealloc_static = on_dealloc;

        extern "C" fn on_dealloc(ptr: *mut u8, layout: StableLayout) {
            // println!("dealloc {ptr:?}");

            let mut allocs = lock_allocs();
            allocs.remove(&AllocatorPtr(ptr)).unwrap_or_else(|| {
                eprintln!("did not found allocation: {ptr:?}");
                std::process::abort();
            });
        }

        let send_cached_allocs_static: *mut extern "C" fn(SliceAllocatorOp) =
            *lib.get(b"SEND_CACHED_ALLOCS\0").unwrap();
        *send_cached_allocs_static = send_cached_allocs;

        extern "C" fn send_cached_allocs(ops: SliceAllocatorOp) {
            let ops = unsafe { ops.into_slice() };

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

        let exit_deallocation: *mut bool = *lib.get(b"EXIT_DEALLOCATION\0").unwrap();
        if *exit_deallocation {
            panic!(
                "library must be unloaded before calling main \n{}",
                "note: before unloading the library, make sure that all threads are joined (if any were spawned by it)"
            );
        }

        // TODO: use get_stabbied?
        let print: *mut extern "C" fn(Str) = *lib.get(b"PRINT\0").unwrap();
        *print = print_impl;

        type MainFn = extern "C" fn(main_resoure_thread_id: i64);

        let main_fn: MainFn = *lib.get_stabbied(b"main").unwrap();

        // let catch_undwind = std::panic::catch_unwind(|| {
        main_fn(main_thread_id);
        // });
        // println!("main fn catch_undwind: {catch_undwind:?}");

        extern "C" fn print_impl(message: Str) {
            if message.starts_with("fatal error:") {
                let backtrace = std::backtrace::Backtrace::force_capture();
                println!("backtrace: {backtrace}");
            }
            println!("dylib: {message}");
        }

        let call_destructors: extern "C" fn() =
            *lib.get_stabbied(b"run_thread_local_dtors").unwrap();
        call_destructors();

        println!("requesting remaining alloc ops");

        let request_cached_allocs: extern "C" fn() =
            *lib.get_stabbied(b"request_cached_allocs").unwrap();

        request_cached_allocs();

        let mut allocs = lock_allocs();
        println!("deallocating remaining memory ({})", allocs.len());

        let exit_fn: extern "C" fn(SliceAllocation) = *lib.get_stabbied(b"exit").unwrap();

        {
            let allocs = std::mem::take(&mut *allocs);
            let allocs: Box<[Allocation]> = allocs
                .into_iter()
                .map(|(_, allocation)| allocation)
                .collect();
            exit_fn((&*allocs).into());
        }
        drop(allocs);

        // TODO: add detection of detached threads (probably other stuff) which prevents library from unloading
        lib.close().unwrap();
    }
}
