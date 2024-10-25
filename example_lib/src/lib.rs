use std::{
    alloc::Layout,
    ffi::c_void,
    sync::atomic::{AtomicI64, AtomicBool, Ordering},
    backtrace::Backtrace,
    cell::Cell,
};

#[unsafe(no_mangle)]
pub static mut PRINT: unsafe extern "C" fn(&str) = print_placeholder;

#[unsafe(no_mangle)]
pub static mut MAIN_CALLED: bool = false;

unsafe extern "C" fn print_placeholder(_: &str) {
    unreachable!();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");

    MAIN_CALLED = true;
    
    // std::panic::set_hook(Box::new(|info| {
    //     let backtrace = Backtrace::capture();
    //     let panic_message = format!("panic: {info}\nbacktrace:\n{backtrace}");
    //     PRINT(&panic_message);
    //     drop(backtrace);
    //     drop(panic_message);
    // }));

    let result = std::thread::spawn(|| {
        panic!("test");
        // fn stack_overflow() {
        //     stack_overflow();
        // }
        // stack_overflow();
        // std::thread::sleep_ms(2000);
        // V.set(Container(vec![1_u8; 10]));
    }).join();

    PRINT(&format!("thread exited with result: {result:?}"));
}
