# dylib-linux-reload

How to run (linux only):<br>
`cargo build --all`<br>
`cargo run`
 
## Problem
Dynamic library is not unloaded if thread-local variables were used

## Possible solution
1. The destructors of thread-local variables prevented the library from unloading (because they are executed only when the thread exits, and the main thread, well, exits when the whole program is closed)
2. Registration of destructors was replaced by no-op and the library was able to unload (by adding the `RTLD_DEEPBIND` flag and substituting `__cxa_thread_atexit_impl` in the library itself), but the memory of thread-locals was leaking out
3. Implementation of manual call of destructors (but only those destructors belonging to the main thread, other threads use the original `__cxa_thread_atexit_impl`) was looked up from [std](https://github.com/rust-lang/rust/blob/f6e511eec7342f59a25f7c0534f1dbea00d01b14/library/std/src/sys/thread_local/destructors/linux_like.rs#L43)
