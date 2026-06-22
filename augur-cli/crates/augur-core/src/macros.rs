//! Shared utility macros.

/// Creates a trait alias by defining a supertrait with a blanket implementation.
///
/// Provides the same behavior as the unstable `trait_alias` feature on stable
/// Rust. The macro generates a new trait requiring all specified supertraits
/// and a blanket `impl` so any type satisfying those bounds automatically
/// implements the alias.
///
/// Supports visibility modifiers, doc comments, and arbitrary trait bounds
/// including lifetimes and generic parameters. Intended for combining up to
/// five traits into a single bound.
///
/// # Examples
/// ```ignore
/// // Combine Send + Sync + 'static into a single bound.
/// trait_alias! {
///     pub(crate) trait ThreadSafe = Send + Sync + 'static
/// }
///
/// // Use doc comments on the alias.
/// trait_alias! {
///     /// Numeric types supporting basic arithmetic.
///     trait Numeric = Copy + PartialOrd + Default
/// }
///
/// // Private alias with generic bounds.
/// trait_alias! {
///     trait SerdeRoundTrip = serde::Serialize + serde::de::DeserializeOwned
/// }
///
/// // Use as a regular trait bound.
/// fn process<T: ThreadSafe>(item: T) { /* ... */ }
/// ```
#[macro_export]
macro_rules! trait_alias {
    (
        $(#[$meta:meta])*
        $vis:vis trait $name:ident = $($bounds:tt)+
    ) => {
        $(#[$meta])*
        $vis trait $name: $($bounds)+ {}
        impl<__TraitAliasAutoImpl: $($bounds)+> $name for __TraitAliasAutoImpl {}
    };
}

/// Acquire a `std::sync::Mutex` guard, recovering from a poisoned lock by
/// consuming the inner value.  Equivalent to the verbose pattern:
/// `mutex.lock().unwrap_or_else(|p| p.into_inner())`.
///
/// # Example
/// ```ignore
/// let guard = lock_or_recover!(my_mutex);
/// guard.do_work();
/// ```
#[macro_export]
macro_rules! lock_or_recover {
    ($m:expr) => {
        $m.lock().unwrap_or_else(|p| p.into_inner())
    };
}

/// Acquire a `std::sync::RwLock` shared read guard, recovering from a poisoned
/// lock by consuming the inner value.
///
/// # Example
/// ```ignore
/// let guard = read_or_recover!(my_rwlock);
/// let value = guard.some_field;
/// ```
#[macro_export]
macro_rules! read_or_recover {
    ($m:expr) => {
        $m.read().unwrap_or_else(|p| p.into_inner())
    };
}

/// Acquire a `std::sync::RwLock` exclusive write guard, recovering from a
/// poisoned lock by consuming the inner value.
///
/// # Example
/// ```ignore
/// let mut guard = write_or_recover!(my_rwlock);
/// guard.mutate_something();
/// ```
#[macro_export]
macro_rules! write_or_recover {
    ($m:expr) => {
        $m.write().unwrap_or_else(|p| p.into_inner())
    };
}
