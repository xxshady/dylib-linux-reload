use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL, Library};

fn main() {
    load_and_unload();
    println!("----------------------------");
    std::thread::sleep(std::time::Duration::from_millis(2000));
    load_and_unload();
}

fn load_and_unload() {
    unsafe {
        let directory = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let lib_path = format!("target/{directory}/libexample_lib.so");
        println!("path: {lib_path}");

        let lib = Library::open(
            Some(lib_path),
            RTLD_LAZY | RTLD_LOCAL,
        )
        .unwrap();

        let main_called: *mut bool = *lib.get(b"MAIN_CALLED\0").unwrap();
        if *main_called {
            panic!("library is not unloaded");
        }

        type MainFn =
            unsafe extern "C" fn();

        let main_fn: MainFn = *lib.get(b"main\0").unwrap();
        main_fn();

        // no difference
        // lib.close().unwrap();
        drop(lib);
    }
}
