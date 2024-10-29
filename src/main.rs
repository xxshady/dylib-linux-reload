use libc::RTLD_DEEPBIND;
use libloading::os::unix::{Library, RTLD_LAZY, RTLD_LOCAL};

fn main() {
    load_and_unload();
    println!("----------------------------");
    load_and_unload();
}

fn load_and_unload() {
    unsafe {
        let main_thread_id = libc::syscall(libc::SYS_gettid);

        let directory = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let lib_path = format!("target/{directory}/libexample_lib.so");
        println!("path: {lib_path}");

        let lib = Library::open(Some(lib_path), RTLD_LAZY | RTLD_LOCAL | RTLD_DEEPBIND).unwrap();

        let main_called: *mut bool = *lib.get(b"MAIN_CALLED\0").unwrap();
        if *main_called {
            panic!("library is not unloaded");
        }

        let main_fn: unsafe extern "C" fn(main_thread_id: i64) = *lib.get(b"main\0").unwrap();
        main_fn(main_thread_id);
        let unload_fn: unsafe extern "C" fn() = *lib.get(b"unload\0").unwrap();
        unload_fn();

        // no difference
        // lib.close().unwrap();
        drop(lib);
    }
}
