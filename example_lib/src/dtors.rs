use std::{cell::RefCell, sync::LazyLock};

struct Destructors(RefCell<Vec<(*mut u8, unsafe extern "C" fn(*mut u8))>>);

// SAFETY: register & run will only be called from one thread
unsafe impl Send for Destructors {}
unsafe impl Sync for Destructors {}

static DESTRUCTORS: LazyLock<Destructors> = LazyLock::new(|| Destructors(Default::default()));

pub unsafe fn register(obj: *mut u8, dtor: unsafe extern "C" fn(*mut u8)) {
    let mut dtors = DESTRUCTORS.0.try_borrow_mut().unwrap();
    dtors.push((obj, dtor));
}

pub unsafe fn run() {
    loop {
        let mut dtors = DESTRUCTORS.0.try_borrow_mut().unwrap();
        match dtors.pop() {
            Some((obj, dtor)) => {
                drop(dtors);
                unsafe {
                    dtor(obj);
                }
            }
            None => {
                *dtors = Vec::new();
                break;
            }
        }
    }
}
