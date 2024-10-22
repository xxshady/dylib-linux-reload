use std::{
    cell::RefCell,
    ffi::c_void,
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
    thread::ThreadId,
};

include!("../../shared/lib.rs");
use shared::Allocation;

mod custom_alloc;
mod dtors;

static MAIN_THREAD_ID: AtomicI64 = AtomicI64::new(0);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_thread_atexit_impl(
    dtor: unsafe extern "C" fn(*mut c_void),
    obj: *mut c_void,
    dso_symbol: *mut c_void,
) {
    // if we are not in main thread use original __cxa_thread_atexit_impl
    if MAIN_THREAD_ID.load(Ordering::SeqCst) != libc::syscall(libc::SYS_gettid) {
        // from fasterthanlime article
        // https://fasterthanli.me/articles/so-you-want-to-live-reload-rust

        type NextFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
        let original_impl: NextFn = std::mem::transmute(libc::dlsym(
            libc::RTLD_NEXT,
            c"__cxa_thread_atexit_impl".as_ptr(),
        ));

        let dtor = std::mem::transmute::<unsafe extern "C" fn(*mut c_void), *mut c_void>(dtor);

        original_impl(dtor, obj, dso_symbol);
    }
    // otherwise use custom implementation so we can unload them when we
    // no longer need this dynamic library to be loaded
    else {
        // from std (kind of) https://github.com/rust-lang/rust/blob/f6e511eec7342f59a25f7c0534f1dbea00d01b14/library/std/src/sys/thread_local/destructors/linux_like.rs#L53

        // not sure about this transmute (there is transmute in the opposite direction
        // from u8 to c_void in std code so I thought it should also be fine to do it in reverse)
        let dtor = std::mem::transmute::<
            unsafe extern "C" fn(*mut c_void),
            unsafe extern "C" fn(*mut u8),
        >(dtor);
        dtors::register(obj.cast(), dtor);
    }
}

use custom_alloc::CustomAlloc;
use std::alloc::{Layout, System};

#[global_allocator]
static GLOBAL: CustomAlloc = CustomAlloc::new();

// SAFETY: all these statics will be initialized on main thread when
// this dynamic library is loaded and then never change

#[unsafe(no_mangle)]
pub static mut ON_ALLOC: unsafe extern "C" fn(*mut u8, Layout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut ON_DEALLOC: unsafe extern "C" fn(*mut u8, Layout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut ON_ALLOC_ZEROED: unsafe extern "C" fn(*mut u8, Layout) =
    on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut ON_REALLOC: unsafe extern "C" fn(*mut u8, *mut u8, Layout, usize) =
    on_realloc_placeholder;

// SAFETY: only mutated once and will be read from main thread
// (it's also used to check if library was unloaded before calling main function)
#[unsafe(no_mangle)]
pub static mut EXIT_DEALLOCATION: bool = false;

unsafe extern "C" fn on_alloc_dealloc_placeholder(_: *mut u8, _: Layout) {
    unreachable!()
}

unsafe extern "C" fn on_realloc_placeholder(_: *mut u8, _: *mut u8, _: Layout, _: usize) {
    unreachable!()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main(main_thread_id: i64, print: unsafe extern "C" fn(&str)) {
    MAIN_THREAD_ID.store(main_thread_id, Ordering::SeqCst);

    static mut PRINT: unsafe extern "C" fn(&str) = print_placeholder;

    assert!(PRINT == print_placeholder);

    PRINT = print;

    unsafe extern "C" fn print_placeholder(_: &str) {
        unreachable!();
    }

    use std::cell::Cell;
    #[derive(Default)]
    struct Container(Vec<u8>);

    impl Drop for Container {
        fn drop(&mut self) {
            unsafe {
                PRINT(&format!(
                    "drop {:?} {:?}",
                    MAIN_THREAD_ID.load(Ordering::SeqCst),
                    libc::syscall(libc::SYS_gettid)
                ));
            }
        }
    }

    thread_local! {
        static V: Cell<Container> = Cell::new(Container(Vec::new()));
    }

    V.set(Container(vec![1_u8; 10]));

    std::thread::spawn(|| {
        std::thread::sleep_ms(2000);
        // V.set(Container(vec![1_u8; 10]));
    });

    // macro_rules! generate_thread_locals {
    //     ($( $repeat:tt )+) => {
    //         $(
    //             {
    //                 thread_local! {
    //                     static V: Cell<Container> = Cell::new(Container(Vec::new()));
    //                 }

    //                 V.set(Container(vec![1_u8; 10]));
    //                 $repeat;

    //                 std::thread::spawn(|| {
    //                     V.set(Container(vec![1_u8; 10]));
    //                 }).join().unwrap();
    //             }
    //         )+
    //     };
    // }

    // generate_thread_locals!(
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     // 210 ^
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    //     () () () () ()
    // );

    // let reg = Region::new(&GLOBAL);
    // std::mem::forget(vec![0_u8; 10_000_000]);
    // let mut v = vec![1];
    // drop(v);

    // let main_thread_vec = vec![1];
    // for _ in 1..=10 {
    // std::thread::spawn(move || {
    //     print("before");
    //     std::thread::sleep_ms(200);
    //     print("after");
    //     // let mut v = vec![1];
    //     // std::mem::forget(v);
    //     drop(main_thread_vec);
    //     print("end");
    // });
    // }

    // static mut V: Vec<u8> = Vec::new();
    // print("before");
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // V.push(1);
    // print("after");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_thread_local_dtors() {
    unsafe {
        dtors::run();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(allocs: &[Allocation]) {
    EXIT_DEALLOCATION = true;
    for Allocation(ptr, layout, ..) in allocs {
        std::alloc::dealloc(*ptr, *layout);
    }
}
