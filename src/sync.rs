//! Defines transaction types which can be sent and shared between threads.
//! 
//! Author --- daniel.bechaz@gmail.com  
//! Last Moddified --- 2019-06-13

use super::*;
use alloc::sync::Arc;
use core::fmt;
#[cfg(feature = "futures",)]
use core::{
  pin::Pin,
  future::Future,
  task::{Poll, Context,},
};
#[cfg(feature = "old-futures",)]
use old_futures::{task, Future, Async, Poll,};

mod rw_lock;
mod tx_box;

use self::{rw_lock::*, tx_box::*,};

/// A guard to an open transaction.
/// 
/// When dropped the transaction will be aborted.
pub struct TxGuard {
  /// The `TxBox` which represents this transaction.
  tx_box: RwTxBox,
}

impl TxGuard {
  /// Attempts to close the transaction.
  /// 
  /// # Params
  /// 
  /// close --- If `true` the transaction state is set to `Closed` otherwise it's set to `Abort`.  
  #[inline]
  fn _close_tx(&mut self, close: bool,) -> TxResult<()> {
    self.tx_box.write().close_tx(close,)
  }
}

impl TxGuard {
  /// Starts a new transaction.
  pub fn new() -> Self {
    Self { tx_box: Arc::new(TxBox::new().into(),), }
  }
  /// Starts a new transaction and adds it as a dependency of this transaction.
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use tx_rs::TxGuard;
  /// 
  /// let mut tx1 = TxGuard::new();
  /// let tx2 = tx1.sub_tx();
  /// 
  /// let (tx1, _) = tx1.close().unwrap_err();
  /// 
  /// tx2.close().unwrap();
  /// tx1.close().unwrap();
  /// ```
  pub fn sub_tx(&mut self,) -> Self {
    let tx = Self::new();

    //Safe because we just created `tx` so there can be no cycle.
    unsafe { self.wait_for_unchecked(&tx,); }

    tx
  }
  /// Attempts to close this transaction.
  /// 
  /// If this transaction cannot be closed for some reason the guard is returned along
  /// with the error.
  pub fn close(mut self,) -> Result<(), (Self, TxError,)> {
    match self._close_tx(true,) {
      //Do not run the destructor.
      Ok(()) => {  
        //Move `tx_box` out and drop it.
        unsafe { core::ptr::read(&self.tx_box,) };
        //Do not run the destructor for `self`.
        core::mem::forget(self,);
        
        Ok(())
      },
      Err(e) => Err((self, e,)),
    }
  }
  /// Attempts to abort this transaction.
  /// 
  /// If this transaction cannot be aborted for some reason the guard is returned along
  /// with the error.
  /// 
  /// This is function will still succeed if there are aborted dependencies.
  pub fn abort(mut self,) -> Result<(), (Self, TxError,)> {
    match self._close_tx(false,) {
      //Do not run the destructor.
      Ok(()) => {  
        //Move `tx_box` out and drop it.
        unsafe { core::ptr::read(&self.tx_box,) };
        //Do not run the destructor for `self`.
        core::mem::forget(self,);
        
        Ok(())
      },
      Err(e) => Err((self, e,)),
    }
  }
  /// Poisons this transaction to indicate it has entered an illegal state.
  /// 
  /// This is the recomended way to handle a `PoisonedError` encounterd while trying to
  /// close this transaction if you are unable to recover using
  /// [`tx.clear_poisoned()`](../struct.TxGuard.html#method.clear_poisoned).
  /// 
  /// If a dependency cycle is causing a deadlock, calling this function on one of the
  /// transactions in the cycle is a way to allow computation to continue assuming all
  /// other transactions properly handle `PoisonError`s.
  pub fn poison(self,) {
    //Get the `tx_box` and avoid running the destructor for `self`.
    let tx_box = {
      //Move `tx_box` out.
      let tx_box = unsafe { core::ptr::read(&self.tx_box,) };
    
      //Do not run the destructor for `self`.
      core::mem::forget(self,);

      tx_box
    };

    tx_box.write().poison();
  }
  /// Adds `other_tx` as a dependency of this transaction.
  /// 
  /// Returns `false` if this transaction is already a dependency of `other_tx` or if
  /// they are the same transaction.
  /// 
  /// Note that this function does allow you to add a transaction as a dependency more
  /// than once.
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use tx_rs::TxGuard;
  /// 
  /// let mut tx1 = TxGuard::new();
  /// let mut tx2 = TxGuard::new();
  /// 
  /// assert!(tx1.wait_for(&tx2));
  /// assert!(!tx2.wait_for(&tx1));
  /// ```
  pub fn wait_for(&mut self, other_tx: &TxRef,) -> bool {
    if other_tx.will_wait(self,) { return false }

    unsafe { self.tx_box.write().wait_for_unchecked(&other_tx.0,); }

    true
  }
  /// Adds `other_tx` as a dependency of this transaction.
  /// 
  /// # Safety
  /// 
  /// This function adds `other_tx` as a dependency without checking if the new
  /// dependency will create a cycle; it is therefor the responsibility of the caller to
  /// ensure no cycles will be created to avoid deadlocks.
  /// 
  /// In the event that a deadlock is detected, call
  /// [`tx.poison()`](../struct.TxGuard.html#method.poison) to resolve it.
  pub unsafe fn wait_for_unchecked(&mut self, other_tx: &TxRef,) {
    self.tx_box.write().wait_for_unchecked(&other_tx.0,)
  }
  /// Clears the poisoned state of this transaction and resets it to opened.
  /// 
  /// This function has no effect when the transaction is not in a poisoned state.
  /// 
  /// Calling `close`/`abort` after clearing the poisoned state will still poison this transaction as many times as there are poisoned dependencies.  
  /// The poisoned state will need to be cleared each time.
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use tx_rs::TxGuard;
  /// 
  /// let mut tx1 = TxGuard::new();
  /// 
  /// tx1.sub_tx().poison();
  /// 
  /// let (mut tx1, _) = tx1.close().unwrap_err();
  /// 
  /// tx1.clear_poisoned();
  /// tx1.close().unwrap();
  /// ```
  /// 
  /// # Safety
  /// 
  /// It is the responsibility to of the caller to handle the poisoned dependency so that
  /// this transaction can continue normally.
  pub fn clear_poisoned(&mut self,) { self.tx_box.write().clear_poisoned() }
  /// Returns a [TxRef] for this transaction.
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use tx_rs::TxGuard;
  /// 
  /// let mut tx1 = TxGuard::new();
  /// let tx2 = tx1.sub_tx().tx_ref();
  /// 
  /// assert!(tx1.will_wait(&tx2));
  /// ```
  #[inline]
  pub fn tx_ref(&self,) -> TxRef { TxRef(self.tx_box.clone(),) }
}

impl Drop for TxGuard {
  #[inline]
  fn drop(&mut self,) { self._close_tx(false,).ok(); }
}

impl core::ops::Deref for TxGuard {
  type Target = TxRef;

  #[inline]
  fn deref(&self,) -> &Self::Target {
    //Safe because both types are wrappers around a `RwTxBox`.
    unsafe { core::mem::transmute(self,) }
  }
}

impl PartialEq for TxGuard {
  #[inline]
  fn eq(&self, rhs: &Self,) -> bool { alloc::sync::Arc::ptr_eq(&self.tx_box, &rhs.tx_box,) }
}

impl Eq for TxGuard {}

impl PartialEq<TxRef> for TxGuard {
  #[inline]
  fn eq(&self, rhs: &TxRef,) -> bool { rhs == self }
}

impl AsRef<TxRef> for TxGuard {
  #[inline]
  fn as_ref(&self,) -> &TxRef { self }
}

impl core::borrow::Borrow<TxRef> for TxGuard {
  #[inline]
  fn borrow(&self,) -> &TxRef { self }
}

impl fmt::Debug for TxGuard {
  #[inline]
  fn fmt(&self, fmt: &mut fmt::Formatter,) -> fmt::Result {
    write!(fmt, concat!(stringify!(TxGuard,), "({:p})",), self.tx_box,)
  }
}

/// A reference to a transaction.
#[derive(Clone,)]
pub struct TxRef(RwTxBox,);

impl TxRef {
  /// Returns the current state of the transaction.
  #[inline]
  pub fn tx_state(&self,) -> TxState { self.0.read().tx_state() }
  /// Returns `true` if `other_tx` is a dependecy of `self` or if `other_tx` is self.
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use tx_rs::TxGuard;
  /// 
  /// let mut tx1 = TxGuard::new();
  /// assert!(tx1.will_wait(&tx1,));
  /// 
  /// let tx2 = tx1.sub_tx();
  /// assert!(tx1.will_wait(&tx2,));
  /// ```
  pub fn will_wait(&self, other_tx: &TxRef,) -> bool {
    self.0.read().will_wait(&other_tx.0.read(),)
  }
}

#[cfg(feature = "futures",)]
impl Future for TxRef {
  type Output = TxState;

  /// Returns `Poll::Pending` as long as the transaction is open.
  fn poll(self: Pin<&mut Self>, context: &mut Context,) -> Poll<Self::Output> {
    match self.tx_state() {
      TxState::Open => {
        //Queue this task to be notified once the transaction is resolved.
        self.0.write().notify(context.waker().clone(),);

        Poll::Pending
      },
      state => Poll::Ready(state),
    }
  }
}

#[cfg(feature = "old-futures",)]
impl Future for TxRef {
  type Item = TxState;
  type Error = !;

  /// Returns `Async::NotReady` as long as the transaction is open.
  fn poll(&mut self,) -> Poll<Self::Item, Self::Error> {
    Ok(match self.tx_state() {
      TxState::Open => {
        //Queue this task to be notified once the transaction is resolved.
        self.0.write().notify(task::current(),);

        Async::NotReady
      },
      state => Async::Ready(state),
    })
  }
}

impl PartialEq for TxRef {
  #[inline]
  fn eq(&self, rhs: &Self,) -> bool { alloc::sync::Arc::ptr_eq(&self.0, &rhs.0,) }
}

impl Eq for TxRef {}

impl PartialEq<TxGuard> for TxRef {
  #[inline]
  fn eq(&self, rhs: &TxGuard,) -> bool { self == rhs.as_ref() }
}

#[cfg(test,)]
mod tests {
  use super::*;

  #[test]
  fn test_tx_guard() {
    //---test single transaction---
    let tx1 = TxGuard::new();
    assert_eq!(tx1.close(), Ok(()), "Error closing single transaction",);

    let mut tx1 = TxGuard::new();
    tx1.clear_poisoned();
    assert_eq!(tx1.tx_state(), TxState::Open,
      "`clear_poisoned` had an effect when it should do nothing",
    );

    let tx1 = TxGuard::new();
    let rtx1 = tx1.tx_ref();

    tx1.poison();
    assert_eq!(rtx1.tx_state(), TxState::Poisoned, "`poison` did not poison the transaction",);

    //---test transactions with dependencies---

    let mut tx1 = TxGuard::new();
    let mut tx2 = TxGuard::new();

    assert!(!tx1.will_wait(&tx2,), "`would_wait` `true` for non dependency",);
    assert!(tx1.wait_for(&tx2,), "Could not make `tx2` a dependency of `tx1`",);
    assert!(tx1.will_wait(&tx2,), "`would_wait` `false` for dependency",);
    assert!(!tx2.wait_for(&tx1,), "Could make `tx1` a dependency of `tx2`",);
    assert!(tx1.close().is_err(), "Closed `tx1` while `tx2` was still open",);
    
    let mut tx1 = TxGuard::new();
    let tx2 = tx1.sub_tx();

    assert!(tx2.close().is_ok(), "Error closing `tx2`",);
    assert!(tx1.close().is_ok(), "Error closing `tx1` after `tx2` was closed",);

    let mut tx1 = TxGuard::new();
    let tx2 = tx1.sub_tx();

    assert!(tx2.abort().is_ok(), "Error aborting `tx2`",);
    assert!(tx1.close().is_err(), "Closed `tx1` while `tx2` was aborted",);

    let mut tx1 = TxGuard::new();
    let tx2 = tx1.sub_tx();

    tx2.abort().ok();
    assert!(tx1.abort().is_ok(), "Error aborting `tx1` after `tx2` was aborted",);

    let mut tx1 = TxGuard::new();
    let tx2 = tx1.sub_tx();

    tx2.poison();
    let mut tx1 = match tx1.close() {
      Ok(()) => panic!("Closed `tx1` while `tx2` was poisoned",),
      Err((tx1, _,)) => tx1,
    };
    tx1.clear_poisoned();
    assert!(tx1.close().is_ok(), "Error closing `tx1` after clearing the poison state",);
  }
  #[test]
  fn test_dependency_cycle_behaviour() {
    let mut tx1 = TxGuard::new();

    assert!(tx1.will_wait(&tx1,), "`will_wait` is `false` for `self`",);

    //Create cycle
    let mut tx2 = tx1.sub_tx();
    unsafe { tx2.wait_for_unchecked(&tx1,); }

    assert!(tx1.close().is_err(), "Closed `tx1` while `tx2` was open",);
    assert!(tx2.close().is_err(), "Closed `tx2` while `tx1` was open",);
  }
}
