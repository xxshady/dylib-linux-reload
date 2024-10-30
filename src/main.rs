use std::{fs, io::stdin, path::PathBuf, time::Duration};

use libloading::os::unix::{Library, RTLD_LAZY, RTLD_LOCAL};

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
    // load_and_unload();
}

fn load_and_unload() {
    unsafe {
        let directory = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let lib_path = format!("target/{directory}/libexample_lib.so");
        dbg!(&lib_path, &PathBuf::from(&lib_path).parent());

        let copy_path = PathBuf::from(&lib_path)
            .parent()
            .unwrap()
            .join("kgjkfjgfgkjhjekw.so");

        fs::copy(&lib_path, &copy_path).unwrap();

        let lib = Library::open(Some(copy_path), RTLD_LAZY | RTLD_LOCAL).unwrap();

        let main_called: *mut bool = *lib.get(b"MAIN_CALLED\0").unwrap();
        if *main_called {
            panic!("library is not unloaded");
        }

        let print_fn: *mut unsafe extern "C" fn(&str) = *lib.get(b"PRINT\0").unwrap();

        *print_fn = print_impl;

        unsafe extern "C" fn print_impl(msg: &str) {
            println!("dylib: {msg}");
        }

        type MainFn = unsafe extern "C" fn();

        let main_fn: MainFn = *lib.get(b"main\0").unwrap();
        main_fn();

        lib.close().unwrap();
        // std::mem::forget(lib);
    }
}
