use std::{
    cell::RefCell,
    ffi::c_void,
    sync::atomic::{AtomicI64, Ordering},
};

mod dtors;

thread_local! {
    static INSTANCE: RefCell<Option<String>> = Default::default();
}

static MAIN_THREAD_ID: AtomicI64 = AtomicI64::new(0);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_thread_atexit_impl(
    dtor: unsafe extern "C" fn(*mut c_void),
    obj: *mut c_void,
    dso_symbol: *mut c_void,
) {
    // if we are not in main thread use original __cxa_thread_atexit_impl
    if MAIN_THREAD_ID.load(Ordering::SeqCst) != libc::syscall(libc::SYS_gettid) {
        type NextFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
        let original_impl: NextFn = std::mem::transmute(libc::dlsym(
            libc::RTLD_NEXT,
            c"__cxa_thread_atexit_impl".as_ptr(),
        ));

        let dtor = std::mem::transmute::<unsafe extern "C" fn(*mut c_void), *mut c_void>(dtor);

        original_impl(dtor, obj, dso_symbol);
    } else {
        let dtor = std::mem::transmute::<
            unsafe extern "C" fn(*mut c_void),
            unsafe extern "C" fn(*mut u8),
        >(dtor);
        dtors::register(obj.cast(), dtor);
    }
}

#[no_mangle]
pub fn main(main_thread_id: i64) {
    MAIN_THREAD_ID.store(main_thread_id, Ordering::SeqCst);

    // this thread local will be deallocated by custom impl (see dtors module)
    INSTANCE.with_borrow_mut(|content| {
        dbg!(content.is_some()); // checking if dylib was unloaded
        content.replace(alloc_a_lot_of_memory());
    });

    std::thread::spawn(|| {
        thread_local! {
            static INSTANCE: RefCell<Option<String>> = Default::default();
        }
        // this thread local will be deallocated by original __cxa_thread_atexit_impl (see above main thread check in custom impl)
        INSTANCE.with_borrow_mut(|content| {
            content.replace(alloc_a_lot_of_memory());
        });
    })
    .join()
    .unwrap();
}

#[no_mangle]
pub fn unload() {
    unsafe {
        dtors::run();
    }
}

fn alloc_a_lot_of_memory() -> String {
    "1".to_string().repeat(1_000_000)
}
