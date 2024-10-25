use std::{
    alloc::{GlobalAlloc, Layout, System},
    ops,
    sync::{Mutex, MutexGuard, atomic::{AtomicIsize, AtomicUsize, AtomicBool, Ordering}},
};

use crate::shared::{Allocation, CLayout};

#[derive(Default, Debug)]
pub struct CustomAlloc {
    inner: System,
}

impl CustomAlloc {
    pub const fn new() -> Self {
        CustomAlloc { inner: System }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // send_buffered_allocs_if_panic_finished();

        let ptr = self.inner.alloc(layout);

        let c_layout = crate::shared::CLayout {
            size: layout.size(),
            align: layout.align(),
        };
        if !crate::CAPTURING_BACKTRACE.load(Ordering::SeqCst) {
            crate::ON_ALLOC(
                ptr,
                c_layout,
            );
        }

        // if !std::thread::panicking() {
            // crate::ON_ALLOC(
            //     ptr,
            //     c_layout,
            // );
        // } else {
            // save_alloc_in_buffer(ptr, c_layout);
        // }
        

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // send_buffered_allocs_if_panic_finished();

        self.inner.dealloc(ptr, layout);

        let c_layout = crate::shared::CLayout {
            size: layout.size(),
            align: layout.align(),
        };
        if !(crate::EXIT_DEALLOCATION || crate::CAPTURING_BACKTRACE.load(Ordering::SeqCst)) {
            crate::ON_DEALLOC(
                ptr,
                c_layout,
            );
        }

        // if !std::thread::panicking() {
            // if !crate::EXIT_DEALLOCATION {
            //     crate::ON_DEALLOC(
            //         ptr,
            //         c_layout,
            //     );
            // }
        // } else {
            // save_dealloc_in_buffer(ptr, c_layout);
        // }
    }

    // unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    //     send_buffered_allocs_if_panic_finished();

    //     let ptr = self.inner.alloc_zeroed(layout);

    //     let c_layout = crate::shared::CLayout {
    //         size: layout.size(),
    //         align: layout.align(),
    //     };
    //     if !std::thread::panicking() {
    //         crate::ON_ALLOC_ZEROED(
    //             ptr,
    //             c_layout,
    //         );
    //     } else {
    //         save_alloc_zeroed_in_buffer(ptr, c_layout);
    //     }

    //     ptr
    // }

    // unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    //     send_buffered_allocs_if_panic_finished();

    //     let new_ptr = self.inner.realloc(ptr, layout, new_size);

    //     let c_layout = crate::shared::CLayout {
    //         size: layout.size(),
    //         align: layout.align(),
    //     };
    //     if !std::thread::panicking() {
    //         crate::ON_REALLOC(
    //             ptr,
    //             new_ptr,
    //             c_layout,
    //             new_size,
    //         );
    //     } else {
    //         save_realloc_in_buffer(ptr, new_ptr, c_layout, new_size);
    //     }

    //     new_ptr
    // }
}

// TODO: what if panic will try to deallocate allocations which were allocated before panic has started?
// const ALLOCS_BUFFER_SIZE: usize = 50_000;
// /// buffer for allocations which happened during a panic
// static ALLOCS_BUFFER: Mutex<Vec<Allocation>> = Mutex::new(Vec::new());

// struct Allocs(heapless::Vec<Allocation, ALLOCS_BUFFER_SIZE>);

// unsafe impl Send for Allocs {}
// unsafe impl Sync for Allocs {}

// static mut ALLOCS_BUFFER: Allocs = Allocs(heapless::Vec::new());

// static ALLOCS_BUFFER_INIT: AtomicBool = AtomicBool::new(false);

// fn lock_allocs_buffer() -> MutexGuard<'static, Vec<Allocation>> {
//     let Ok(allocs) = ALLOCS_BUFFER.try_lock() else {
//         unsafe {
//             crate::PRINT("fatal error: failed to lock ALLOCS_BUFFER");
//         }
//         std::process::abort();
//     };
//     allocs
// }
// fn lock_allocs_buffer() -> &'static mut heapless::Vec<Allocation, ALLOCS_BUFFER_SIZE> {
//     unsafe {
//         &mut ALLOCS_BUFFER.0
//     }
// }

// fn push_to_allocs_buffer(allocation: Allocation) {
//     let mut allocs = lock_allocs_buffer();
    
//     if allocs.len() == ALLOCS_BUFFER_SIZE {
//         unsafe {
//             crate::PRINT("fatal error: ALLOCS_BUFFER is full");
//         }
//         std::process::abort();
//     }

//     allocs.push(allocation).unwrap();
// }

// fn send_buffered_allocs_if_panic_finished() {
//     if ALLOCS_BUFFER_INIT.load(Ordering::SeqCst) {
//         return;
//     }

//     if std::thread::panicking() {
//         return;
//     }

//     let mut allocs = &mut *lock_allocs_buffer();
//     if allocs.is_empty() {
//         return;
//     }

//     for Allocation(ptr, layout) in allocs.iter() {
//         unsafe {
//             crate::ON_ALLOC(
//                 *ptr,
//                 crate::shared::CLayout {
//                     size: layout.size,
//                     align: layout.align,
//                 },
//             );
//         }
//     }

//     allocs.clear();
// }

// fn save_alloc_in_buffer(ptr: *mut u8, layout: CLayout) {
//     push_to_allocs_buffer(Allocation(ptr, layout));
// }

// fn save_dealloc_in_buffer(ptr: *mut u8, layout: CLayout) {
//     let mut allocs = lock_allocs_buffer();

//     const NOT_FOUND: usize = ALLOCS_BUFFER_SIZE + 1; 
//     let mut idx = NOT_FOUND;
//     let mut iteration = 0_usize;
//     for Allocation(ptr_, ..) in allocs.iter() {
//         if *ptr_ != ptr {
//             iteration += 1;
//             continue;
//         }

//         idx = iteration;
//         break;
//     }
//     if idx == NOT_FOUND {
//         allocation_not_found();
//     }

//     unsafe {
//         allocs.swap_remove_unchecked(idx);
//     }
// }

// fn save_alloc_zeroed_in_buffer(ptr: *mut u8, layout: CLayout) {
//     push_to_allocs_buffer(Allocation(ptr, layout));
// }

// fn save_realloc_in_buffer(
//     ptr: *mut u8,
//     new_ptr: *mut u8,
//     layout: CLayout,
//     new_size: usize,
// ) {
//     save_dealloc_in_buffer(ptr, layout);
//     let new_layout = CLayout {
//         size: new_size,
//         align: layout.align,
//     };
//     save_alloc_in_buffer(new_ptr, new_layout);
//     // let mut allocs = lock_allocs_buffer();

//     // let old_allocation = Allocation(ptr, layout);
//     // let el = allocs.iter_mut().find(|allocation| {
//     //     return **allocation == old_allocation;
//     // });
//     // let Some(el) = el else {
//     //     allocation_not_found();
//     // };

//     // let new_layout = CLayout {
//     //     size: new_size,
//     //     align: layout.align,
//     // };
//     // *el = Allocation(new_ptr, new_layout);
// }

// fn allocation_not_found() -> ! {
//     // TODO: improve error message but be careful about allocations!!!
//     unsafe {
//         crate::PRINT("unknown allocation");
//     }
//     std::process::abort();
// }

// pub unsafe fn init() {
//     ALLOCS_BUFFER_INIT.swap(true, Ordering::SeqCst);
//     // let mut allocs = ALLOCS_BUFFER.try_lock().expect("must never happen");
//     let allocs = lock_allocs_buffer();
//     // allocs.reserve(ALLOCS_BUFFER_SIZE);
//     ALLOCS_BUFFER_INIT.swap(false, Ordering::SeqCst);
// }
