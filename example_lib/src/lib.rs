use std::{
    alloc::Layout,
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicBool, Ordering}},
    cell::Cell,
};

include!("../../shared/lib.rs");
use shared::{Allocation, CLayout, AllocatorOp, AllocatorPtr};

mod custom_alloc;
use custom_alloc::CustomAlloc;

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

#[global_allocator]
static GLOBAL: CustomAlloc = CustomAlloc::new();

// SAFETY: all these statics will be initialized on main thread when
// this dynamic library is loaded and then never change

#[unsafe(no_mangle)]
pub static mut ON_ALLOC: unsafe extern "C" fn(*mut u8, CLayout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut ON_DEALLOC: unsafe extern "C" fn(*mut u8, CLayout) = on_alloc_dealloc_placeholder;

#[unsafe(no_mangle)]
pub static mut SEND_CACHED_ALLOCS: unsafe extern "C" fn(&[AllocatorOp]) = send_cached_allocs_placeholder;

unsafe extern "C" fn send_cached_allocs_placeholder(_: &[AllocatorOp]) {
    unreachable!();
}

// TODO: use AtomicBool
// SAFETY: only mutated once and will be read from main thread
// (it's also used to check if library was unloaded before calling main function)
#[unsafe(no_mangle)]
pub static mut EXIT_DEALLOCATION: bool = false;

unsafe extern "C" fn on_alloc_dealloc_placeholder(_: *mut u8, _: CLayout) {
    unreachable!()
}

// SAFETY: only mutated once from main thread
#[unsafe(no_mangle)]
pub static mut PRINT: unsafe extern "C" fn(&str) = print_placeholder;

unsafe extern "C" fn print_placeholder(_: &str) {
    unreachable!();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main(main_thread_id: i64) {
    std::env::set_var("RUST_BACKTRACE", "1");
    
    // PRINT("before init");
    custom_alloc::init();
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

        // TEST
        // PRINT(&panic_message);
        std::hint::black_box(&panic_message);

        drop(backtrace);
        drop(panic_message);
        // backtrace::clear_symbol_cache();
        // CAPTURING_BACKTRACE.swap(false, Ordering::SeqCst);
    }));

    // ignoring result on purpose because panic is handled in the custom panic hook
    // let _ = std::panic::catch_unwind(|| {
    //     struct Bomb;
    //     impl Drop for Bomb {
    //         fn drop(&mut self) {
    //             panic!("boom");
    //         }
    //     }
    //     let _boom = Bomb;
    //     panic!("test");
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

    // static mut PRINT: unsafe extern "C" fn(&str) = print_placeholder;

    // assert!(PRINT == print_placeholder);

    // PRINT = print;

    // unsafe extern "C" fn print_placeholder(_: &str) {
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

    std::thread::scope(|s| {
        let handles = std::array::from_fn::<_, 10, _>(|_| {
            s.spawn(|| {
                panic!("test");
            })
        });

        for h in handles {
            let result = h.join();
            PRINT(&format!("thread exited with result: {result:?}"));
        }
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
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_thread_local_dtors() {
    println!("calling thread-local destructors ({})", dtors::len());

    dtors::run();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(allocs: &[Allocation]) {
    EXIT_DEALLOCATION = true;

    for Allocation(AllocatorPtr(ptr), layout, ..) in allocs {
        std::alloc::dealloc(
            *ptr,
            Layout::from_size_align(layout.size, layout.align).unwrap(),
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn request_cached_allocs() {
    custom_alloc::send_cached_allocs(None);
}
