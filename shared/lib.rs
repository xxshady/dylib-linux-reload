mod shared {
    use std::fmt::{Debug, Formatter, Result as FmtResult};

    #[derive(Clone, PartialEq)]
    pub struct Allocation(pub *mut u8, pub CLayout);

    // SAFETY: `*mut u8` won't be touched anywhere except in the dynamic library in the main thread for deallocation
    unsafe impl Send for Allocation {}
    unsafe impl Sync for Allocation {}

    impl Debug for Allocation {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "({:?}, {:?})", self.0, self.1.size)
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub struct CLayout {
        pub size: usize,
        pub align: usize,
    }
}
