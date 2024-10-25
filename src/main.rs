use libloading::os::unix::{RTLD_LAZY, RTLD_LOCAL};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
        MutexGuard,
    },
    thread::ThreadId,
};

fn main() {
    for _ in 1..=10 {
        load_and_unload();
        println!("----------------------------");
    }
}

fn load_and_unload() {
    unsafe {
        let directory = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let lib = libloading::os::unix::Library::open(
            Some(format!("target/{directory}/libexample_lib.so")),
            RTLD_LAZY | RTLD_LOCAL,
        )
        .unwrap();

        let main_called: *mut bool = *lib.get(b"MAIN_CALLED\0").unwrap();
        if *main_called {
            panic!("library must be unloaded before calling main");
        }

        let print: *mut unsafe extern "C" fn(&str) = *lib.get(b"PRINT\0").unwrap();
        *print = print_impl;

        type MainFn =
            unsafe extern "C" fn();

        let main_fn: MainFn = *lib.get(b"main\0").unwrap();

        main_fn();

        unsafe extern "C" fn print_impl(message: &str) {
            println!("dylib: {message}");
        }

        drop(lib);
    }
}
