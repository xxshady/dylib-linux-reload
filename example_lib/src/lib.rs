use std::{collections::HashMap, hash::RandomState, sync::Mutex};

#[unsafe(no_mangle)]
pub static mut MAIN_CALLED: bool = false;

#[unsafe(no_mangle)]
pub static mut PRINT: unsafe extern "C" fn(&str) = print_placeholder;

unsafe extern "C" fn print_placeholder(_: &str) {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() {
    MAIN_CALLED = true;

    PRINT("5");

    static ALLOCS_CACHE: Mutex<Option<HashMap<i32, i32>>> = Mutex::new(None);

    ALLOCS_CACHE
        .lock()
        .unwrap()
        .replace(HashMap::with_capacity(20_000));
}
