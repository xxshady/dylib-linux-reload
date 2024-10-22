use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};
use std::{
    alloc::Layout,
    cell::Cell,
    fmt::{Debug, Formatter, Result as FmtResult},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread::ThreadId,
};

use std::ffi::c_void;

include!("../shared/lib.rs");
use shared::Allocation;

fn main() {
    for _ in 1..=2 {
        load_and_unload();
        println!("----------------------------");
        // TEST
        RESOURCE_HAS_BEEN_SHUTDOWN.store(false, Ordering::SeqCst);
    }
}

// TODO: store existing resource *instances* (with instance ids) in *multi-threaded* structure
// (not thread-local, because we need to access it from memory allocator)
// to check if they still valid
static RESOURCE_HAS_BEEN_SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn load_and_unload() {
    unsafe {
        // I could use `std::thread::current().id()`
        // but I'm not sure how safe it is for FFI (+ it needs to be stored in a static)
        // since it's an opaque object and as_u64() is unstable
        let main_thread_id = libc::syscall(libc::SYS_gettid);

        // this flag allows us to replace __cxa_thread_atexit_impl in dynamic library
        const RTLD_DEEPBIND: i32 = 0x00008;
        let lib = libloading::os::unix::Library::open(
            Some("target/debug/libexample_lib.so"),
            RTLD_LAZY | RTLD_LOCAL | RTLD_DEEPBIND,
        )
        .unwrap();

        static ALLOCS: Mutex<Vec<Allocation>> = Mutex::new(Vec::new());

        let on_alloc_static: *mut unsafe extern "C" fn(*mut u8, Layout) =
            *lib.get(b"ON_ALLOC\0").unwrap();
        *on_alloc_static = on_alloc;

        unsafe extern "C" fn on_alloc(ptr: *mut u8, layout: Layout) {
            let thread_id = std::thread::current().id();

            println!("alloc {ptr:?} {thread_id:?}");
            // dbg!(ptr, layout, libc::syscall(libc::SYS_gettid));

            let mut allocs = ALLOCS.lock().expect("should never happen");
            allocs.push(Allocation(ptr, layout));
        }

        let on_dealloc_static: *mut unsafe extern "C" fn(*mut u8, Layout) =
            *lib.get(b"ON_DEALLOC\0").unwrap();
        *on_dealloc_static = on_dealloc;

        unsafe extern "C" fn on_dealloc(ptr: *mut u8, layout: Layout) {
            println!("dealloc {ptr:?}");
            // dbg!(ptr, layout);

            let mut allocs = ALLOCS.lock().expect("should never happen");

            let old_allocation = Allocation(ptr, layout);
            let el = allocs.iter().enumerate().find(|(idx, allocation)| {
                return **allocation == old_allocation;
            });
            let Some((idx, _)) = el else {
                panic!("did not found allocation: {ptr:?} {layout:?}")
            };

            allocs.swap_remove(idx);
        }

        let on_alloc_zeroed_static: *mut unsafe extern "C" fn(*mut u8, Layout) =
            *lib.get(b"ON_ALLOC_ZEROED\0").unwrap();
        *on_alloc_zeroed_static = on_alloc_zeroed;

        unsafe extern "C" fn on_alloc_zeroed(ptr: *mut u8, layout: Layout) {
            println!("alloc_zeroed");

            let mut allocs = ALLOCS.lock().expect("should never happen");
            allocs.push(Allocation(ptr, layout));
        }

        let on_realloc_static: *mut unsafe extern "C" fn(*mut u8, *mut u8, Layout, usize) =
            *lib.get(b"ON_REALLOC\0").unwrap();
        *on_realloc_static = on_realloc;

        unsafe extern "C" fn on_realloc(
            ptr: *mut u8,
            new_ptr: *mut u8,
            layout: Layout,
            new_size: usize,
        ) {
            println!("realloc");
            // dbg!(ptr, new_ptr, layout, new_size);

            let mut allocs = ALLOCS.lock().expect("should never happen");

            let old_allocation = Allocation(ptr, layout);
            let el = allocs.iter_mut().find(|allocation| {
                return **allocation == old_allocation;
            });
            let Some(el) = el else {
                panic!("did not found allocation: {ptr:?} {layout:?}")
            };

            let new_layout =
                Layout::from_size_align(new_size, layout.align()).expect("should never happen");
            *el = Allocation(new_ptr, new_layout);
        }

        let resource_main_thread_id = std::thread::current().id();

        let exit_deallocation: *mut bool = *lib.get(b"EXIT_DEALLOCATION\0").unwrap();
        if *exit_deallocation {
            panic!(
                "library must be unloaded before calling main \n{}",
                "note: before unloading the library, make sure that all threads are joined (if any were spawned by it)"
            );
        }

        type MainFn =
            unsafe extern "C" fn(main_resoure_thread_id: i64, print: unsafe extern "C" fn(&str));

        let main_fn: MainFn = *lib.get(b"main\0").unwrap();
        main_fn(main_thread_id, print_impl);

        unsafe extern "C" fn print_impl(message: &str) {
            println!("dylib: {message}");
        }

        type CallThreadLocalDestructorsFn = unsafe extern "C" fn();

        let call_destructors: CallThreadLocalDestructorsFn =
            *lib.get(b"run_thread_local_dtors\0").unwrap();
        call_destructors();

        RESOURCE_HAS_BEEN_SHUTDOWN.store(true, Ordering::SeqCst);

        let mut allocs = ALLOCS.lock().expect("should never happen");

        let exit_fn: unsafe extern "C" fn(&[Allocation]) = *lib.get(b"exit\0").unwrap();
        exit_fn(dbg!(&allocs));

        *allocs = Vec::new();
        drop(allocs);

        // libloading crate will call dlclose in Drop implementation for us
        // (explicit drop call for clarity)
        drop(lib);
    }
}
