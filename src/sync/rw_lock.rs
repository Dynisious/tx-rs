//! Author --- daniel.bechaz@gmail.com  
//! Last Moddified --- 2019-06-13

use core::{
  ops::{Deref, DerefMut,},
  borrow::{Borrow, BorrowMut,},
  sync::atomic::{
    self,
    AtomicUsize,
    Ordering,
  },
};

const WRITE_BIT: usize = 0b1;
const ADD_READER: usize = 0b10;
const SUB_READER: usize = !WRITE_BIT;

/// A basic Read-Write lock using spin locking.
pub(super) struct RwLock<T,> {
  value: T,
  state: AtomicUsize,
}

impl<T,> RwLock<T,> {
  /// Creates a new `RwLock`.
  pub const fn new(value: T,) -> Self {
    Self {
      value,
      state: AtomicUsize::new(0,),
    }
  }
  /// Aquires a read only lock.
  /// 
  /// # Safety
  /// 
  /// It is possible to cause a deadlock if the thread already holds a [WriteGuard].
  pub fn read(&self,) -> ReadGuard<T,> {
    loop {
      let state = self.state.load(Ordering::Acquire,);

      if state & WRITE_BIT == 0 {
        if self.state.compare_and_swap(state, state + ADD_READER, Ordering::Release,) == state {
          return ReadGuard { lock: self, }
        }
      }

      atomic::spin_loop_hint();
    }
  }
  /// Aquires a write lock.
  /// 
  /// # Safety
  /// 
  /// It is possible to cause a deadlock if the thread already holds a [WriteGuard].
  pub fn write(&self,) -> WriteGuard<T,> {
    //Get ownership of the write bit.
    loop {
      let state = self.state.load(Ordering::Acquire,);

      if state & WRITE_BIT == 0 {
        if self.state.compare_and_swap(state, state | WRITE_BIT, Ordering::Release,) == state {
          break
        }
      }

      atomic::spin_loop_hint();
    }
    
    //Wait for other users to release their locks.
    loop {
      atomic::spin_loop_hint();

      let state = self.state.load(Ordering::Acquire,);

      if state == WRITE_BIT {
        return WriteGuard { lock: self, }
      }
    }
  }
}

impl<T,> From<T,> for RwLock<T,> {
  #[inline]
  fn from(from: T,) -> Self { Self::new(from,) }
}

pub(super) struct ReadGuard<'lock, T,> {
  lock: &'lock RwLock<T,>,
}

impl<T,> Drop for ReadGuard<'_, T,> {
  fn drop(&mut self,) {
    loop {
      atomic::spin_loop_hint();

      let state = self.lock.state.load(Ordering::Acquire,);

      if self.lock.state.compare_and_swap(state, state.wrapping_add(SUB_READER,), Ordering::Release,) == state {
        return
      }
    }
  }
}

impl<T,> Deref for ReadGuard<'_, T,> {
  type Target = T;

  #[inline]
  fn deref(&self,) -> &Self::Target { &self.lock.value }
}

impl<T,> AsRef<T,> for ReadGuard<'_, T,> {
  #[inline]
  fn as_ref(&self,) -> &T { self }
}

impl<T,> Borrow<T,> for ReadGuard<'_, T,> {
  #[inline]
  fn borrow(&self,) -> &T { self }
}

pub(super) struct WriteGuard<'lock, T,> {
  lock: &'lock RwLock<T,>,
}

impl<T,> Drop for WriteGuard<'_, T,> {
  fn drop(&mut self,) {
    //Release the write lock.
    self.lock.state.store(0, Ordering::Release,)
  }
}

impl<T,> Deref for WriteGuard<'_, T,> {
  type Target = T;

  #[inline]
  fn deref(&self,) -> &Self::Target { &self.lock.value }
}

impl<T,> DerefMut for WriteGuard<'_, T,> {
  #[inline]
  fn deref_mut(&mut self,) -> &mut Self::Target {
    unsafe { core::mem::transmute(&self.lock.value as *const _,) }
  }
}

impl<T,> AsRef<T,> for WriteGuard<'_, T,> {
  #[inline]
  fn as_ref(&self,) -> &T { self }
}

impl<T,> AsMut<T,> for WriteGuard<'_, T,> {
  #[inline]
  fn as_mut(&mut self,) -> &mut T { self }
}

impl<T,> Borrow<T,> for WriteGuard<'_, T,> {
  #[inline]
  fn borrow(&self,) -> &T { self }
}

impl<T,> BorrowMut<T,> for WriteGuard<'_, T,> {
  #[inline]
  fn borrow_mut(&mut self,) -> &mut T { self }
}
