#[unsafe(no_mangle)]
pub static mut MAIN_CALLED: bool = false;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() {
    MAIN_CALLED = true;

    // prevents unloading     
    std::thread::scope(|s| {
        std::thread::spawn(|| {}).join().unwrap();
    });
    // works fine
    // std::thread::spawn(|| {}).join().unwrap();
}
