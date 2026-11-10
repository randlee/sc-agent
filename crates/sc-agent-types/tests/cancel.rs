//! CancelToken (REQ-SAG-0104 boundary part, REQ-SAG-0601 wake primitives) and the Clock waits
//! over it.
//!
//! Deterministic by design: time moves only through a ManualClock the test advances, each
//! interleaving is forced once through the token's `testing` trace points, and every wait for
//! another thread is bounded by the testkit's hang guard. No test reads real time.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sc_agent_testkit::{Gate, HANG_GUARD, join_within, recv_within};
use sc_agent_types::testing::{CancelEvent, ManualClock};
use sc_agent_types::{CancelHook, CancelToken, Clock, Wait};

/// A deadline far enough away that only an explicit advance reaches it.
const LONG: Duration = Duration::from_secs(60);

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
    assert_send_sync::<Clock>();
}

/// A token cancelled on another thread while the wait is parked (the waiter registers with
/// the clock under the token's lock, and the cancel needs that lock, which the waiter releases
/// only by parking) ends the wait with `Cancelled`; the clock never moved, so the deadline
/// cannot be what ended it.
#[test]
fn test_cancel_from_another_thread_ends_a_parked_wait() {
    let manual = ManualClock::new();
    let clock = manual.clock();
    let token = CancelToken::new();
    let waiter = token.clone();
    let deadline = clock.now() + LONG;
    let handle = thread::spawn(move || clock.wait_until(&waiter, deadline));
    assert!(manual.wait_for_waiters(1), "the waiter parked");
    token.cancel();
    assert_eq!(
        join_within(handle, HANG_GUARD).expect("waiter"),
        Wait::Cancelled
    );
    assert_eq!(manual.waiters(), 0, "the wait deregistered from the clock");
}

/// A parked wait ends `DeadlineReached` when the clock is advanced to the deadline.
#[test]
fn test_wait_ends_when_the_clock_reaches_the_deadline() {
    let manual = ManualClock::new();
    let clock = manual.clock();
    let deadline = clock.now() + LONG;
    let handle = thread::spawn(move || clock.wait_until(&CancelToken::new(), deadline));
    assert!(manual.wait_for_waiters(1), "the waiter parked");
    manual.advance(LONG);
    assert_eq!(
        join_within(handle, HANG_GUARD).expect("waiter"),
        Wait::DeadlineReached
    );
}

/// A deadline 1 ms ahead of a clock that does not move is never reached: the waiter parks and
/// only the cancel (sent once it is parked) ends the wait. Deterministic: the deadline check
/// does not depend on when the test runs, only on the clock reading.
#[test]
fn test_a_deadline_just_ahead_is_not_reached_before_the_clock_gets_there() {
    let manual = ManualClock::new();
    let clock = manual.clock();
    let token = CancelToken::new();
    let waiter = token.clone();
    let deadline = clock.now() + std::time::Duration::from_millis(1);
    let handle = thread::spawn(move || clock.wait_until(&waiter, deadline));
    assert!(manual.wait_for_waiters(1), "the waiter registered");
    token.cancel();
    assert_eq!(
        join_within(handle, HANG_GUARD).expect("waiter"),
        Wait::Cancelled,
        "1 ms short of the deadline is not the deadline"
    );
}

#[test]
fn test_wait_returns_at_once_when_cancelled_or_past_the_deadline() {
    let manual = ManualClock::new();
    let clock = manual.clock();
    let token = CancelToken::new();
    assert_eq!(clock.wait_until(&token, clock.now()), Wait::DeadlineReached);
    token.cancel();
    assert_eq!(
        clock.wait_until(&token, clock.now() + LONG),
        Wait::Cancelled
    );
    assert_eq!(manual.waiters(), 0, "neither wait parked");
}

/// The real clock: an already-cancelled token and a past deadline return at once, without a
/// real wait (nothing here takes time).
#[test]
fn test_system_clock_returns_at_once_when_cancelled_or_past_the_deadline() {
    let clock = Clock::system();
    let token = CancelToken::new();
    assert_eq!(clock.wait_until(&token, clock.now()), Wait::DeadlineReached);
    token.cancel();
    assert_eq!(
        clock.wait_until(&token, clock.now() + LONG),
        Wait::Cancelled
    );
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
    recv_within(&woken, HANG_GUARD);
    join_within(handle, HANG_GUARD).expect("cancel thread");
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
    let handle = thread::spawn(move || canceller.cancel());
    join_within(handle, HANG_GUARD).expect("cancel finished (no deadlock)");
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
    let clock = ManualClock::new().clock();
    assert_eq!(
        clock.wait_until(&token, clock.now() + LONG),
        Wait::Cancelled
    );
    assert!(format!("{token:?}").contains("cancelled: true"));
}

/// REQ-SAG-0104 "never after its CancelHook is dropped", the interleaving where the drop comes
/// while `cancel` runs the hook on another thread, forced once: `cancel` is held right after it
/// took the hook (trace `HookTaken`), the drop is started and reports that it waits for the
/// running hook (trace `DropWaiting`), and only then is `cancel` let go. The drop returns only
/// after the hook's side effect, so the effect never appears after the drop returned.
#[test]
fn test_a_drop_during_the_hook_waits_for_it_to_finish() {
    let token = CancelToken::new();
    let (events, seen) = mpsc::channel::<&'static str>();
    let taken = Gate::new();
    let release = Gate::new();
    let opener = release.opener();
    {
        let events = Mutex::new(events.clone());
        let (taken, release) = (taken.clone(), release.clone());
        token.set_trace(Arc::new(move |event| match event {
            CancelEvent::HookTaken => {
                taken.open();
                release.wait_open();
            }
            CancelEvent::DropWaiting => {
                let _ = events.lock().expect("events").send("drop waiting");
            }
            _ => {}
        }));
    }
    let effect = Arc::new(AtomicUsize::new(0));
    let hook_effect = Arc::clone(&effect);
    let hook_events = Mutex::new(events.clone());
    let hook = token.on_cancel(Arc::new(move || {
        hook_effect.store(1, Ordering::SeqCst);
        let _ = hook_events.lock().expect("events").send("hook ran");
    }));
    let canceller_token = token.clone();
    let canceller = thread::spawn(move || canceller_token.cancel());
    taken.wait_open();
    let dropper = thread::spawn(move || {
        drop(hook);
        let _ = events.send("drop returned");
    });
    assert_eq!(recv_within(&seen, HANG_GUARD), "drop waiting");
    assert_eq!(effect.load(Ordering::SeqCst), 0, "the hook has not run yet");
    opener.open();
    assert_eq!(recv_within(&seen, HANG_GUARD), "hook ran");
    assert_eq!(recv_within(&seen, HANG_GUARD), "drop returned");
    join_within(dropper, HANG_GUARD).expect("dropper");
    join_within(canceller, HANG_GUARD).expect("canceller");
}

/// The other interleaving, forced once: the drop returns before `cancel` takes the hook, so
/// the hook never runs.
#[test]
fn test_a_drop_before_the_cancel_means_the_hook_never_runs() {
    let token = CancelToken::new();
    let (count, hook) = counter();
    let handle = token.on_cancel(hook);
    drop(handle);
    let canceller = token.clone();
    join_within(thread::spawn(move || canceller.cancel()), HANG_GUARD).expect("canceller");
    assert_eq!(count.load(Ordering::SeqCst), 0);
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
    join_within(thread::spawn(move || canceller.cancel()), HANG_GUARD)
        .expect("cancel finished (no self-deadlock)");
    assert_eq!(ran_count.load(Ordering::SeqCst), 1);
    assert!(own.lock().expect("own handle").is_none());
}

/// A panicking hook does not leave a later drop waiting forever.
#[test]
fn test_drop_after_a_panicking_hook_returns() {
    let token = CancelToken::new();
    let hook = token.on_cancel(Arc::new(|| panic!("hook failed (expected in this test)")));
    let canceller = token.clone();
    assert!(join_within(thread::spawn(move || canceller.cancel()), HANG_GUARD).is_err());
    join_within(thread::spawn(move || drop(hook)), HANG_GUARD).expect("drop returned");
}
