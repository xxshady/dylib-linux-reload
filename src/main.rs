use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};

fn main() {
    for _ in 1..=2 {
        unsafe {
            // I could use `std::thread::current().id()`
            // but I'm not sure how safe it is for FFI (+ it needs to be stored in a static)
            // since it's an opaque object  and as_u64() is unstable
            let main_thread_id = libc::syscall(libc::SYS_gettid);

            // this flag allows us to replace __cxa_thread_atexit_impl in dynamic library
            const RTLD_DEEPBIND: i32 = 0x00008;
            let lib = libloading::os::unix::Library::open(
                Some("target/debug/libexample_lib.so"),
                RTLD_LAZY | RTLD_LOCAL | RTLD_DEEPBIND,
            )
            .unwrap();
            let main_fn: unsafe extern "C" fn(main_thread_id: i64) = *lib.get(b"main\0").unwrap();
            main_fn(main_thread_id);
            let unload_fn: unsafe extern "C" fn() = *lib.get(b"unload\0").unwrap();
            unload_fn();

            // libloading crate will call dlclose in Drop implementation for us
            // (explicit drop call for clarity)
            drop(lib);
        }
    }
}
