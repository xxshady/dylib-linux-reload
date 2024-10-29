use std::thread::{current, Thread};
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    },
};

/// A scope to spawn scoped threads in.
///
/// See [`scope`] for details.
// #[stable(feature = "scoped_threads", since = "1.63.0")]
pub struct Scope<'scope, 'env: 'scope> {
    data: Arc<ScopeData>,
    /// Invariance over 'scope, to make sure 'scope cannot shrink,
    /// which is necessary for soundness.
    ///
    /// Without invariance, this would compile fine but be unsound:
    ///
    /// ```compile_fail,E0373
    /// std::thread::scope(|s| {
    ///     s.spawn(|| {
    ///         let a = String::from("abcd");
    ///         s.spawn(|| println!("{a:?}")); // might run after `a` is dropped
    ///     });
    /// });
    /// ```
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

/// An owned permission to join on a scoped thread (block on its termination).
///
/// See [`Scope::spawn`] for details.
// #[stable(feature = "scoped_threads", since = "1.63.0")]
// pub struct ScopedJoinHandle<'scope, T>(JoinInner<'scope, T>);

pub(super) struct ScopeData {
    num_running_threads: AtomicUsize,
    a_thread_panicked: AtomicBool,
    main_thread: Thread,
}

impl ScopeData {
    // pub(super) fn increment_num_running_threads(&self) {
    //     // We check for 'overflow' with usize::MAX / 2, to make sure there's no
    //     // chance it overflows to 0, which would result in unsoundness.
    //     if self.num_running_threads.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
    //         // This can only reasonably happen by mem::forget()'ing a lot of ScopedJoinHandles.
    //         self.overflow();
    //     }
    // }

    // #[cold]
    // fn overflow(&self) {
    //     self.decrement_num_running_threads(false);
    //     panic!("too many running threads in thread scope");
    // }

    // pub(super) fn decrement_num_running_threads(&self, panic: bool) {
    //     if panic {
    //         self.a_thread_panicked.store(true, Ordering::Relaxed);
    //     }
    //     if self.num_running_threads.fetch_sub(1, Ordering::Release) == 1 {
    //         self.main_thread.unpark();
    //     }
    // }
}

pub fn scope<'env, F, T>(f: F)
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    // We put the `ScopeData` into an `Arc` so that other threads can finish their
    // `decrement_num_running_threads` even after this function returns.
    let scope = Scope {
        data: Arc::new(ScopeData {
            num_running_threads: AtomicUsize::new(0),
            main_thread: current(),
            a_thread_panicked: AtomicBool::new(false),
        }),
        env: PhantomData,
        scope: PhantomData,
    };

    // f(&scope)
}
