mod shared {
    use std::fmt::{Debug, Formatter, Result as FmtResult};

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct AllocatorPtr(pub *mut u8);

    // SAFETY: `*mut u8` won't be touched anywhere except in the dynamic library in the main thread for deallocation
    unsafe impl Send for AllocatorPtr {}
    unsafe impl Sync for AllocatorPtr {}

    #[derive(Clone, PartialEq)]
    pub struct Allocation(pub AllocatorPtr, pub CLayout);

    impl Debug for Allocation {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            let Self(AllocatorPtr(ptr), CLayout { size, .. }) = self;
            write!(f, "({:?}, {:?})", ptr, size)
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub struct CLayout {
        pub size: usize,
        pub align: usize,
    }
    
    #[repr(C)]
    #[derive(Clone, PartialEq, Debug)]
    pub enum AllocatorOp {
        Alloc(Allocation),
        Dealloc(Allocation),
    }
}
