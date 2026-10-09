//! Background work for the identity form: Go ran each operation in a
//! goroutine and polled a channel. Desktop uses a thread per job; the
//! browser, which has no threads, spawns the future on the JS event loop.
//! Results come back through a channel the panel polls each frame.

/// `Send` where threads exist; nothing in the single-threaded browser.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F: Future<Output = ()> + MaybeSend + 'static>(job: F) {
    std::thread::spawn(move || block_on(job));
}

#[cfg(target_arch = "wasm32")]
pub fn spawn<F: Future<Output = ()> + MaybeSend + 'static>(job: F) {
    wasm_bindgen_futures::spawn_local(job);
}

/// Drives a future on the current thread. The desktop transport blocks
/// instead of pending, so this mostly returns on the first poll.
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<F: Future>(future: F) -> F::Output {
    use std::{
        pin::pin,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        thread::Thread,
    };
    struct Unpark(Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}
