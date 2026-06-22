use augur_core::{lock_or_recover, read_or_recover, trait_alias, write_or_recover};
use std::fmt::Debug;
use std::sync::{Mutex, RwLock};

const ARBITRARY_I32: i32 = 42;
const ARBITRARY_U64: u64 = 42;

trait_alias! {
    /// Alias combining Debug and Clone.
    trait DebugClone = Debug + Clone
}

trait_alias! {
    pub(crate) trait SendSyncStatic = Send + Sync + 'static
}

trait_alias! {
    trait CopyDefault = Copy + Default
}

#[test]
fn alias_is_implemented_for_qualifying_types() {
    fn assert_debug_clone<T: DebugClone>(_: &T) {}

    assert_debug_clone(&ARBITRARY_I32);
    assert_debug_clone(&String::from("hello"));
    assert_debug_clone(&vec![1, 2, 3]);
}

#[test]
fn alias_works_as_trait_bound() {
    fn needs_send_sync<T: SendSyncStatic>(_: T) {}

    needs_send_sync(ARBITRARY_U64);
    needs_send_sync(String::from("thread-safe"));
}

#[test]
fn alias_with_copy_default() {
    fn make_default<T: CopyDefault>() -> T {
        T::default()
    }

    let x: i32 = make_default();
    assert_eq!(x, 0);

    let y: f64 = make_default();
    assert!((y - 0.0).abs() < f64::EPSILON);
}

#[test]
fn alias_can_be_used_in_where_clause() {
    fn process<T>(val: T) -> String
    where
        T: DebugClone,
    {
        format!("{:?}", val)
    }

    assert_eq!(process(42), "42");
}

#[test]
fn alias_is_usable_as_generic_constraint() {
    fn collect_debug<T: DebugClone>(val: &T) -> String {
        let cloned = val.clone();
        format!("{:?}", cloned)
    }

    assert_eq!(collect_debug(&99_i32), "99");
}

fn poison_mutex<T>(mutex: &Mutex<T>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = mutex.lock().expect("lock mutex before poisoning");
        panic!("poison mutex for test");
    }));
    assert!(result.is_err(), "poison helper should panic");
}

fn poison_rwlock<T>(lock: &RwLock<T>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = lock.write().expect("lock rwlock before poisoning");
        panic!("poison rwlock for test");
    }));
    assert!(result.is_err(), "poison helper should panic");
}

#[test]
fn lock_or_recover_acquires_healthy_mutex() {
    let mutex = Mutex::new(vec![1, 2]);

    let mut guard = lock_or_recover!(mutex);
    guard.push(3);

    assert_eq!(*guard, vec![1, 2, 3]);
}

#[test]
fn lock_or_recover_recovers_from_poisoned_mutex() {
    let mutex = Mutex::new(vec![1, 2]);
    poison_mutex(&mutex);

    let mut guard = lock_or_recover!(mutex);
    guard.push(3);

    assert_eq!(*guard, vec![1, 2, 3]);
}

#[test]
fn read_or_recover_acquires_healthy_rwlock() {
    let lock = RwLock::new(String::from("ready"));

    let guard = read_or_recover!(lock);

    assert_eq!(guard.as_str(), "ready");
}

#[test]
fn read_or_recover_recovers_from_poisoned_rwlock() {
    let lock = RwLock::new(String::from("ready"));
    poison_rwlock(&lock);

    let guard = read_or_recover!(lock);

    assert_eq!(guard.as_str(), "ready");
}

#[test]
fn write_or_recover_acquires_healthy_rwlock() {
    let lock = RwLock::new(String::from("ready"));

    let mut guard = write_or_recover!(lock);
    guard.push_str("-set");

    assert_eq!(guard.as_str(), "ready-set");
}

#[test]
fn write_or_recover_recovers_from_poisoned_rwlock() {
    let lock = RwLock::new(String::from("ready"));
    poison_rwlock(&lock);

    let mut guard = write_or_recover!(lock);
    guard.push_str("-set");

    assert_eq!(guard.as_str(), "ready-set");
}

#[test]
fn lock_or_recover_recovers_poisoned_mutex_guard() {
    let lock = std::sync::Arc::new(std::sync::Mutex::new(7usize));
    let lock_for_panic = std::sync::Arc::clone(&lock);
    let _ = std::thread::spawn(move || {
        let _guard = lock_for_panic.lock().expect("acquire lock");
        panic!("poison lock for recovery path");
    })
    .join();

    let guard = lock_or_recover!(lock);
    assert_eq!(*guard, 7usize);
}

#[test]
fn read_and_write_macros_recover_poisoned_rwlock_guards() {
    let lock = std::sync::Arc::new(std::sync::RwLock::new(3usize));
    let lock_for_panic = std::sync::Arc::clone(&lock);
    let _ = std::thread::spawn(move || {
        let mut guard = lock_for_panic.write().expect("acquire write lock");
        *guard = 9usize;
        panic!("poison rwlock for recovery path");
    })
    .join();

    {
        let mut write_guard = write_or_recover!(lock);
        *write_guard += 1usize;
    }
    let read_guard = read_or_recover!(lock);
    assert_eq!(*read_guard, 10usize);
}
