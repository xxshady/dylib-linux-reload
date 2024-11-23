use std::{
    alloc::Layout,
    ffi::c_void,
    sync::atomic::{AtomicI64, Ordering},
};

use stabby::str::Str;

use shared::{Allocation, AllocatorPtr, SliceAllocation, SliceAllocatorOp, StableLayout};

mod custom_alloc;
use custom_alloc::CustomAlloc;

mod dtors;

static MAIN_THREAD_ID: AtomicI64 = AtomicI64::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn __cxa_thread_atexit_impl(
    dtor: extern "C" fn(*mut c_void),
    obj: *mut c_void,
    dso_symbol: *mut c_void,
) {
    unsafe {
        // if we are not in main thread use original __cxa_thread_atexit_impl
        if MAIN_THREAD_ID.load(Ordering::SeqCst) != libc::syscall(libc::SYS_gettid) {
            // from fasterthanlime article
            // https://fasterthanli.me/articles/so-you-want-to-live-reload-rust

            type NextFn = extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
            let original_impl: NextFn = std::mem::transmute(libc::dlsym(
                libc::RTLD_NEXT,
                c"__cxa_thread_atexit_impl".as_ptr(),
            ));

            let dtor = std::mem::transmute::<extern "C" fn(*mut c_void), *mut c_void>(dtor);

            original_impl(dtor, obj, dso_symbol);
        }
        // otherwise use custom implementation so we can unload them when we
        // no longer need this dynamic library to be loaded
        else {
            // from std (kind of) https://github.com/rust-lang/rust/blob/f6e511eec7342f59a25f7c0534f1dbea00d01b14/library/std/src/sys/thread_local/destructors/linux_like.rs#L53

            // not sure about this transmute (there is transmute in the opposite direction
            // from u8 to c_void in std code so I thought it should also be fine to do it in reverse)
            let dtor =
                std::mem::transmute::<extern "C" fn(*mut c_void), extern "C" fn(*mut u8)>(dtor);
            dtors::register(obj.cast(), dtor);
        }
    }
}

#[global_allocator]
static GLOBAL: CustomAlloc = CustomAlloc::new();

// SAFETY: all these statics will be initialized on main thread when
// this dynamic library is loaded and then never change

#[unsafe(no_mangle)]
pub static mut ON_ALLOC: extern "C" fn(*mut u8, StableLayout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut ON_DEALLOC: extern "C" fn(*mut u8, StableLayout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut SEND_CACHED_ALLOCS: extern "C" fn(SliceAllocatorOp) = send_cached_allocs_placeholder;

extern "C" fn send_cached_allocs_placeholder(_: SliceAllocatorOp) {
    unreachable!();
}

// TODO: use AtomicBool
// SAFETY: only mutated once and will be read from main thread
// (it's also used to check if library was unloaded before calling main function)
#[unsafe(no_mangle)]
pub static mut EXIT_DEALLOCATION: bool = false;

extern "C" fn on_alloc_dealloc_placeholder(_: *mut u8, _: StableLayout) {
    unreachable!()
}

// SAFETY: only mutated once from main thread
#[unsafe(no_mangle)]
pub static mut PRINT: extern "C" fn(Str) = print_placeholder;

extern "C" fn print_placeholder(_: Str) {
    unreachable!();
}

#[stabby::export]
pub extern "C" fn main(main_thread_id: i64) {
    MAIN_THREAD_ID.store(main_thread_id, Ordering::SeqCst);

    std::env::set_var("RUST_BACKTRACE", "1");

    // PRINT("before init");
    unsafe {
        custom_alloc::init();
    }
    // PRINT("after init");

    // double free test (set CACHE_SIZE to 2) --------------------
    // let mut vec1 = vec![1_u8];
    // let vec2 = vec![1_u16];
    // std::mem::forget(vec2);
    // drop(vec1);
    // let mut vec1 = vec![1_u8];
    // drop(vec1);
    // return;
    // -------------------------------------

    // TODO: make it more similar to default panic hook (for example, output thread name)
    std::panic::set_hook(Box::new(|info| {
        // CAPTURING_BACKTRACE.swap(true, Ordering::SeqCst);

        // TODO: check env variables? (RUST_BACKTRACE and friends)
        // TODO: use std backtrace?
        let backtrace = backtrace::Backtrace::new();
        let panic_message = format!("panic: {info}\nbacktrace:\n{backtrace:?}");

        unsafe {
            PRINT(panic_message.as_str().into());
        }
        // // TEST
        // std::hint::black_box(&panic_message);

        drop(backtrace);
        drop(panic_message);
        // backtrace::clear_symbol_cache();
        // CAPTURING_BACKTRACE.swap(false, Ordering::SeqCst);
    }));

    // ignoring result on purpose because panic is handled in the custom panic hook
    // let _ = std::panic::catch_unwind(|| {
    //     // struct Bomb;
    //     // impl Drop for Bomb {
    //     //     fn drop(&mut self) {
    //     //         panic!("boom");
    //     //     }
    //     // }
    //     // let _boom = Bomb;
    //     panic!("test panic");
    // });

    // if let Err(e) = res {
    // let e = e.downcast_ref::<&str>().unwrap();
    // let backtrace = if let Some(backtrace) = CURRENT_BACKTRACE.take() {
    //     backtrace.to_string()
    // } else {
    //     "<no backtrace>".to_string()
    // };
    // PRINT(&format!("catch unwind error: {e}, backtrace:\n{backtrace}"));
    // }
    // let mut vector = vec![];

    // for _ in 1..100 {
    //     vector.push(1_u8);
    // }

    // vector.shrink_to_fit();
    // std::mem::forget(vector);

    // panic!("test");

    // MAIN_THREAD_ID.store(main_thread_id, Ordering::SeqCst);

    // static mut PRINT: extern "C" fn(&str) = print_placeholder;

    // assert!(PRINT == print_placeholder);

    // PRINT = print;

    // extern "C" fn print_placeholder(_: &str) {
    //     unreachable!();
    // }

    // use std::cell::Cell;
    // #[derive(Default)]
    // struct Container(Vec<u8>);

    // impl Drop for Container {
    //     fn drop(&mut self) {
    //         unsafe {
    //             PRINT(&format!(
    //                 "drop {:?} {:?}",
    //                 MAIN_THREAD_ID.load(Ordering::SeqCst),
    //                 libc::syscall(libc::SYS_gettid)
    //             ));
    //         }
    //     }
    // }

    // thread_local! {
    //     static V: Cell<Container> = Cell::new(Container(Vec::new()));
    // }

    // V.set(Container(vec![1_u8; 10]));

    // let handles = std::array::from_fn::<_, 10, _>(|_| {
    //     s.spawn(|| {
    //         panic!("test");
    //     })
    // });

    // for h in handles {
    //     let result = h.join();
    //     PRINT(&format!("thread exited with result: {result:?}"));
    // }

    std::thread::scope(|_s| {
        // s.spawn(|| {
        //     // panic!("test");
        // });
        // let handles = std::array::from_fn::<_, 3, _>(|_| {
        //     s.spawn(|| {
        //         panic!("test");
        //     })
        // });

        // for h in handles {
        //     let result = h.join();
        //     PRINT(
        //         format!("thread exited with result: {result:?}")
        //             .as_str()
        //             .into(),
        //     );
        // }
    });

    // let result = std::thread::spawn(|| {
    //     panic!("test");
    //     // fn stack_overflow() {
    //     //     stack_overflow();
    //     // }
    //     // stack_overflow();
    //     // std::thread::sleep_ms(2000);
    //     // V.set(Container(vec![1_u8; 10]));
    // }).join();

    // PRINT(&format!("thread exited with result: {result:?}"));

    // // macro_rules! generate_thread_locals {
    // //     ($( $repeat:tt )+) => {
    // //         $(
    // //             {
    // //                 thread_local! {
    // //                     static V: Cell<Container> = Cell::new(Container(Vec::new()));
    // //                 }

    // //                 V.set(Container(vec![1_u8; 10]));
    // //                 $repeat;

    // //                 std::thread::spawn(|| {
    // //                     V.set(Container(vec![1_u8; 10]));
    // //                 }).join().unwrap();
    // //             }
    // //         )+
    // //     };
    // // }

    // // generate_thread_locals!(
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     // 210 ^
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // //     () () () () ()
    // // );

    // // let reg = Region::new(&GLOBAL);
    // // std::mem::forget(vec![0_u8; 10_000_000]);
    // // let mut v = vec![1];
    // // drop(v);

    // // let main_thread_vec = vec![1];
    // // for _ in 1..=10 {
    // // std::thread::spawn(move || {
    // //     print("before");
    // //     std::thread::sleep_ms(200);
    // //     print("after");
    // //     // let mut v = vec![1];
    // //     // std::mem::forget(v);
    // //     drop(main_thread_vec);
    // //     print("end");
    // // });
    // // }

    // // static mut V: Vec<u8> = Vec::new();
    // // print("before");
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // V.push(1);
    // // print("after");

    // TODO: add tests for file handles, network ports
    // {
    //     static mut V: Option<std::net::TcpListener> = None;
    //     let v = std::net::TcpListener::bind("127.0.0.1:9999").unwrap();
    //     unsafe {
    //         V = Some(v);
    //     }
    // }
}

#[stabby::export]
pub extern "C" fn run_thread_local_dtors() {
    unsafe {
        PRINT(
            format!("calling thread-local destructors ({})", dtors::len())
                .as_str()
                .into(),
        );
        dtors::run();
    }
}

#[stabby::export]
pub extern "C" fn exit(allocs: SliceAllocation) {
    let allocs = unsafe { allocs.into_slice() };

    unsafe {
        EXIT_DEALLOCATION = true;
    }

    for Allocation(AllocatorPtr(ptr), layout, ..) in allocs {
        unsafe {
            std::alloc::dealloc(
                *ptr,
                Layout::from_size_align(layout.size, layout.align).unwrap(),
            );
        }
    }
}

#[stabby::export]
pub extern "C" fn request_cached_allocs() {
    custom_alloc::send_cached_allocs(None);
}
