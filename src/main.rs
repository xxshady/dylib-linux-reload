use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};

fn main() {
    for _ in 1..=2 {
        unsafe {
            let main_thread_id = libc::syscall(libc::SYS_gettid);

            let lib = libloading::os::unix::Library::open(
                Some("target/debug/libexample_lib.so"),
                RTLD_LAZY | RTLD_LOCAL | 0x00008, // RTLD_DEEPBIND
            )
            .unwrap();
            let main_fn: unsafe extern "C" fn(main_thread_id: i64) =
                *lib.get(b"main\0").unwrap();
            main_fn(main_thread_id);
            let unload_fn: unsafe extern "C" fn() = *lib.get(b"unload\0").unwrap();
            unload_fn();
        }
    }
}
