#![no_std]

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
pub use spinning_top::Spinlock;

#[derive(Debug)]
pub struct LockError(&'static str);

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to acquire {}: already in use", self.0)
    }
}

pub struct UniqueLock<T> {
    name: &'static str,
    mutex: Spinlock<T>,
}

pub type UniqueGuard<'a, T> = spinning_top::SpinlockGuard<'a, T>;

impl<T> UniqueLock<T> {
    pub const fn new(name: &'static str, val: T) -> UniqueLock<T> {
        UniqueLock {
            name,
            mutex: spinning_top::const_spinlock(val),
        }
    }
    pub fn lock(&self) -> Result<UniqueGuard<T>, LockError> {
        self.mutex.try_lock().ok_or(LockError(self.name))
    }
    pub fn is_locked(&self) -> bool {
        self.mutex.is_locked()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum OnceError {
    NotInit,
    AlreadyInit,
}

impl fmt::Display for OnceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OnceError::NotInit => write!(f, "not initialized"),
            OnceError::AlreadyInit => write!(f, "already initialized"),
        }
    }
}

// TODO verify safety and atomic correctness
pub struct UniqueOnce<T> {
    initialized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send + Sync> Sync for UniqueOnce<T> {}
unsafe impl<T: Send> Send for UniqueOnce<T> {}

impl<T> UniqueOnce<T> {
    pub const fn new() -> UniqueOnce<T> {
        UniqueOnce {
            initialized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
    pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> Result<(), OnceError> {
        match self
            .initialized
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(false) => {
                let val = f();
                unsafe {
                    (*self.data.get()).as_mut_ptr().write(val);
                }
                Ok(())
            }
            _ => Err(OnceError::AlreadyInit),
        }
    }
    pub fn is_completed(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
    pub fn get(&self) -> Result<&T, OnceError> {
        match self.initialized.load(Ordering::Acquire) {
            true => Ok(unsafe { &*(*self.data.get()).as_ptr() }),
            false => Err(OnceError::NotInit),
        }
    }
}

impl<T> Drop for UniqueOnce<T> {
    fn drop(&mut self) {
        if *self.initialized.get_mut() {
            unsafe {
                self.data.get_mut().assume_init_drop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unique_lock() {
        let lock = UniqueLock::new("test lock", 14);
        assert!(!lock.is_locked());
        {
            let mut val = lock.lock().unwrap();
            assert!(lock.is_locked());
            *val += 1;
        }
        assert!(!lock.is_locked());
        {
            let val = lock.lock().unwrap();
            assert_eq!(*val, 15);
        }
        assert!(!lock.is_locked());
        {
            let g1 = lock.lock();
            let g2 = lock.lock();
            assert!(g2.is_err());
            assert!(g1.is_ok());
        }
        assert!(!lock.is_locked());
    }
    #[test]
    fn unique_once() {
        let once = UniqueOnce::new();
        assert!(!once.is_completed());
        assert_eq!(once.get(), Err(OnceError::NotInit));
        assert!(once.call_once(|| 14).is_ok());
        assert!(once.is_completed());
        assert_eq!(once.call_once(|| 15), Err(OnceError::AlreadyInit));
        assert_eq!(once.get(), Ok(&14));
    }
}
