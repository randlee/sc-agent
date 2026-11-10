//! CancelToken (REQ-SAG-0104 boundary part, REQ-SAG-0601 wake primitives). No test sleeps:
//! waits use deadlines and channels.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sc_agent_types::{CancelHook, CancelToken, Wait};

/// Far enough away that reaching it means the cancel did not wake the waiter.
const LONG: Duration = Duration::from_secs(60);
/// "At once": a woken waiter returns well inside this bound.
const PROMPT: Duration = Duration::from_secs(10);

fn counter() -> (Arc<AtomicUsize>, Arc<dyn Fn() + Send + Sync>) {
    let count = Arc::new(AtomicUsize::new(0));
    let hook_count = Arc::clone(&count);
    let hook: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        hook_count.fetch_add(1, Ordering::SeqCst);
    });
    (count, hook)
}

/// REQ-SAG-0104 AC2 (boundary part): the PyO3 crate can wrap the token.
#[test]
fn test_cancel_token_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CancelToken>();
    assert_send_sync::<CancelHook>();
}

/// Iterations of the mid-wait cancel test, each with its own token.
const WAITS: usize = 200;
/// How long the cancelling side waits (on a token nobody cancels) before it cancels, so the
/// waiter has almost certainly parked in `wait_until`. A deadline wait, not a sleep.
const PARK_DELAY: Duration = Duration::from_millis(2);

/// A token cancelled on another thread while `wait_until` is parked ends the wait with
/// `Cancelled` well before the deadline.
#[test]
fn test_cancel_from_another_thread_ends_wait_at_once() {
    let delay = CancelToken::new();
    for _ in 0..WAITS {
        let token = CancelToken::new();
        let waiter = token.clone();
        let (started, wait_started) = mpsc::channel();
        let handle = thread::spawn(move || {
            let start = Instant::now();
            started.send(()).expect("signal start");
            (waiter.wait_until(start + LONG), start.elapsed())
        });
        wait_started.recv().expect("waiter started");
        assert_eq!(
            delay.wait_until(Instant::now() + PARK_DELAY),
            Wait::DeadlineReached
        );
        token.cancel();
        let (wait, elapsed) = handle.join().expect("waiter thread");
        assert_eq!(wait, Wait::Cancelled);
        assert!(elapsed < PROMPT, "took {elapsed:?}");
    }
}

#[test]
fn test_wait_until_returns_cancelled_at_once_when_already_cancelled() {
    let token = CancelToken::new();
    token.cancel();
    let start = Instant::now();
    assert_eq!(token.wait_until(Instant::now() + LONG), Wait::Cancelled);
    assert!(start.elapsed() < PROMPT);
}

#[test]
fn test_wait_until_reaches_deadline_with_no_cancel() {
    let token = CancelToken::new();
    let deadline = Instant::now() + Duration::from_millis(20);
    assert_eq!(token.wait_until(deadline), Wait::DeadlineReached);
    assert!(Instant::now() >= deadline);
    let past = Instant::now();
    assert_eq!(token.wait_until(past), Wait::DeadlineReached);
}

/// Every registered hook runs once on cancel (cancel twice still runs it once); a hook
/// registered after cancel runs at once; a dropped hook never runs.
#[test]
fn test_hooks_run_once_and_dropped_hooks_never_run() {
    let token = CancelToken::new();
    let (first_count, first) = counter();
    let (second_count, second) = counter();
    let (dropped_count, dropped) = counter();
    let _first = token.on_cancel(first);
    let _second = token.clone().on_cancel(second);
    drop(token.on_cancel(dropped));
    assert_eq!(first_count.load(Ordering::SeqCst), 0);

    token.cancel();
    token.cancel();
    assert_eq!(first_count.load(Ordering::SeqCst), 1);
    assert_eq!(second_count.load(Ordering::SeqCst), 1);
    assert_eq!(dropped_count.load(Ordering::SeqCst), 0);

    let (late_count, late) = counter();
    let _late = token.on_cancel(late);
    assert_eq!(late_count.load(Ordering::SeqCst), 1);
    token.cancel();
    assert_eq!(late_count.load(Ordering::SeqCst), 1);
}

/// A hook runs on cancel from another thread and can wake a waiter through a channel.
#[test]
fn test_hook_wakes_a_channel_receiver() {
    let token = CancelToken::new();
    let (wake, woken) = mpsc::channel::<()>();
    let wake = Mutex::new(wake);
    let _hook = token.on_cancel(Arc::new(move || {
        wake.lock().expect("sender").send(()).expect("send wake");
    }));
    let canceller = token.clone();
    let handle = thread::spawn(move || canceller.cancel());
    woken.recv_timeout(PROMPT).expect("hook ran");
    handle.join().expect("cancel thread");
}

/// A hook that registers or drops another hook does not deadlock: hooks run outside the lock.
#[test]
fn test_hook_that_registers_or_drops_hooks_does_not_deadlock() {
    let token = CancelToken::new();
    let (inner_count, inner) = counter();
    let (other_count, other) = counter();
    let other_hook = Arc::new(Mutex::new(Some(token.on_cancel(other))));
    let registered: Arc<Mutex<Vec<CancelHook>>> = Arc::new(Mutex::new(Vec::new()));

    let hook_token = token.clone();
    let hook_registered = Arc::clone(&registered);
    let hook_other = Arc::clone(&other_hook);
    let _outer = token.on_cancel(Arc::new(move || {
        let hook = hook_token.on_cancel(Arc::clone(&inner));
        hook_registered.lock().expect("registered").push(hook);
        drop(hook_other.lock().expect("other").take());
    }));

    let canceller = token.clone();
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        canceller.cancel();
        done.send(()).expect("signal done");
    });
    finished
        .recv_timeout(PROMPT)
        .expect("cancel finished (no deadlock)");
    // The inner hook was registered after cancel, so it ran at once.
    assert_eq!(inner_count.load(Ordering::SeqCst), 1);
    assert_eq!(registered.lock().expect("registered").len(), 1);
    // `other` was taken out by cancel before the outer hook dropped its handle: at most once.
    assert!(other_count.load(Ordering::SeqCst) <= 1);
    assert!(other_hook.lock().expect("other").is_none());
}

/// Clones share one flag and compare equal; separate tokens do not.
#[test]
fn test_clones_share_the_flag() {
    let token = CancelToken::default();
    let clone = token.clone();
    let other = CancelToken::new();
    assert_eq!(token, clone);
    assert_ne!(token, other);
    clone.cancel();
    assert!(token.is_cancelled());
    assert!(!other.is_cancelled());
    assert_eq!(token.wait_until(Instant::now() + LONG), Wait::Cancelled);
    assert!(format!("{token:?}").contains("cancelled: true"));
}

/// Iterations of the cancel-versus-drop race; each runs cancel and drop at the same moment.
const RACES: usize = 2000;

/// REQ-SAG-0104 "never after its CancelHook is dropped", under concurrency: `cancel()` on one
/// thread races `drop(hook)` on another. Once drop returns, the hook has either finished or
/// will never run, so its side effect never appears later.
#[test]
fn test_hook_never_runs_after_its_drop_returns() {
    let mut ran = 0;
    let mut late = 0;
    for _ in 0..RACES {
        let token = CancelToken::new();
        let effect = Arc::new(AtomicUsize::new(0));
        let hook_effect = Arc::clone(&effect);
        let hook = token.on_cancel(Arc::new(move || {
            // Widen the window between "taken by cancel" and "side effect" (no sleep).
            for _ in 0..64 {
                thread::yield_now();
            }
            hook_effect.store(1, Ordering::SeqCst);
        }));
        let start = Arc::new(Barrier::new(2));
        let canceller_start = Arc::clone(&start);
        let canceller_token = token.clone();
        let canceller = thread::spawn(move || {
            canceller_start.wait();
            canceller_token.cancel();
        });
        start.wait();
        drop(hook);
        let seen_at_drop = effect.load(Ordering::SeqCst);
        canceller.join().expect("canceller thread");
        let seen_after = effect.load(Ordering::SeqCst);
        if seen_after != seen_at_drop {
            late += 1;
        }
        ran += seen_after;
    }
    assert_eq!(
        late, 0,
        "{late} of {RACES} hooks ran after their drop returned"
    );
    // Not a requirement, only a sign the race was exercised: report how often the hook won.
    eprintln!("hook ran before drop in {ran} of {RACES} races");
}

/// A hook that drops its own `CancelHook` while it runs returns instead of waiting for itself.
#[test]
fn test_hook_dropping_its_own_handle_does_not_deadlock() {
    let token = CancelToken::new();
    let own: Arc<Mutex<Option<CancelHook>>> = Arc::new(Mutex::new(None));
    let (ran_count, ran) = counter();
    let hook_own = Arc::clone(&own);
    let hook = token.on_cancel(Arc::new(move || {
        drop(hook_own.lock().expect("own handle").take());
        ran();
    }));
    *own.lock().expect("own handle") = Some(hook);

    let canceller = token.clone();
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        canceller.cancel();
        done.send(()).expect("signal done");
    });
    finished
        .recv_timeout(PROMPT)
        .expect("cancel finished (no self-deadlock)");
    assert_eq!(ran_count.load(Ordering::SeqCst), 1);
    assert!(own.lock().expect("own handle").is_none());
}

/// A panicking hook does not leave a later drop waiting forever.
#[test]
fn test_drop_after_a_panicking_hook_returns() {
    let token = CancelToken::new();
    let hook = token.on_cancel(Arc::new(|| panic!("hook failed")));
    let canceller = token.clone();
    assert!(thread::spawn(move || canceller.cancel()).join().is_err());
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        drop(hook);
        done.send(()).expect("signal done");
    });
    finished.recv_timeout(PROMPT).expect("drop returned");
}
