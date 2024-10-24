use std::{
    alloc::{GlobalAlloc, Layout, System},
    ops,
    sync::atomic::{AtomicIsize, AtomicUsize, Ordering},
};

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
        let ptr = self.inner.alloc(layout);
        crate::ON_ALLOC(
            ptr,
            crate::shared::CLayout {
                size: layout.size(),
                align: layout.align(),
            },
        );
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.dealloc(ptr, layout);

        if !crate::EXIT_DEALLOCATION {
            crate::ON_DEALLOC(
                ptr,
                crate::shared::CLayout {
                    size: layout.size(),
                    align: layout.align(),
                },
            );
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc_zeroed(layout);
        crate::ON_ALLOC_ZEROED(
            ptr,
            crate::shared::CLayout {
                size: layout.size(),
                align: layout.align(),
            },
        );
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = self.inner.realloc(ptr, layout, new_size);
        crate::ON_REALLOC(
            ptr,
            new_ptr,
            crate::shared::CLayout {
                size: layout.size(),
                align: layout.align(),
            },
            new_size,
        );
        new_ptr
    }
}
