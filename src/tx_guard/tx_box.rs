//! Author --- daniel.bechaz@gmail.com  
//! Last Moddified --- 2019-06-13

use super::*;
use core::{
  cell::Cell,
};
use alloc::{
  rc::Rc,
  vec::Vec,
  collections::VecDeque,
};
#[cfg(feature = "futures",)]
use core::task::Waker;
#[cfg(feature = "old-futures",)]
use old_futures::task::Task;

static DEPS_IN_USE: &str = "`dependencies` already in use";
#[cfg(any(feature = "futures", feature = "old-futures",),)]
static NOTIFY_IN_USE: &str = "`notify` already in use";

pub(super) type RcTxBox = Rc<TxBox>;

/// Represents a transaction.
pub(super) struct TxBox {
  /// The state of the transaction.
  tx_state: Cell<TxState>,
  /// The wakers to notify once this transaction is resolved.
  #[cfg(feature = "futures",)]
  notify: Cell<Option<Vec<Waker>>>,
  /// The wakers to notify once this transaction is resolved.
  #[cfg(feature = "old-futures",)]
  notify: Cell<Option<Vec<Task>>>,
  /// The transactions this transaction is waiting on.
  dependencies: Cell<Option<Vec<RcTxBox>>>,
}

impl TxBox {
  /// Breadth first searches the dependency tree for `other_tx` and returns `true` if it
  /// is found.
  /// 
  /// # Params
  /// 
  /// dependencies --- The queue of dependencies to be checked.  
  /// other_tx --- The transaction to search for.  
  fn _is_dependency(mut dependencies: VecDeque<RcTxBox>, other_tx: &Self,) -> bool {
    while let Some(dep) = dependencies.pop_front() {
      if core::ptr::eq::<Self>(dep.as_ref(), other_tx,) { return true }

      let deps = dep.dependencies.replace(None,)
        .expect(DEPS_IN_USE);
      //Add the dependencies of this dependency to be searched.
      dependencies.extend(deps.iter().map(Clone::clone,),);
      //Replace the dependencies.
      dep.dependencies.set(Some(deps),);
    }

    false
  }
}

impl TxBox {
  /// Creates a new `TxBox`.
  #[inline]
  pub fn new() -> Self {
    Self {
      tx_state: Cell::new(TxState::Open,).into(),
      #[cfg(feature = "futures",)]
      notify: Some(Vec::new()).into(),
      #[cfg(feature = "old-futures",)]
      notify: Some(Vec::new()).into(),
      dependencies: Some(Vec::new()).into(),
    }
  }
  /// Attempts to close this transaction.
  /// 
  /// If a dependency is poisoned: this transaction will be poisoned, the dependency
  /// removed and a poisoned error returned.
  /// 
  /// Else if a dependency is aborted, and we were attempting to close rather than abort:
  /// this transaction will remain open an aborted error returned.
  /// 
  /// Else if a dependency is open: this transaction will remain open and an open
  /// dependency error returned.
  /// 
  /// # Params
  /// 
  /// close --- If `true` the transaction state is set to `Closed` otherwise it's set to `Abort`.  
  pub fn close_tx(&self, close: bool,) -> TxResult<()> {
    match self.tx_state.get() {
      //We've already finished this transaction in some way.
      TxState::Closed | TxState::Aborted => Err(TxError::AlreadyClosed),
      //This transaction is poisoned.
      TxState::Poisoned => Err(TxError::PoisonedError),
      //Attempt to close the transaction.
      TxState::Open => {
        //Get the new state.
        let state = {
          //Get the initial desired state.
          let mut state = if close { TxState::Closed }
            else { TxState::Aborted };

          //Check the dependencies.
          let mut index = 0;
          let mut dependencies = self.dependencies.replace(None,)
            .expect(DEPS_IN_USE,);
          while index < dependencies.len() {
            match dependencies[index].tx_state() {
              //A dependency is aborted, also abort.
              TxState::Aborted => state = TxState::Aborted,
              //A dependency is poisoned, exit immediately.
              TxState::Poisoned => {
                //Poison this transaction.
                self.tx_state.set(TxState::Poisoned,);
                //Remove the dependency.
                dependencies.swap_remove(index,);
                //Replace the dependency.
                self.dependencies.set(Some(dependencies),);

                return Err(TxError::PoisonedError);
              },
              //A dependency is closed, remove it.
              TxState::Closed => {
                dependencies.swap_remove(index,);
                continue;
              }
              //A dependency is open while we're trying to close.
              TxState::Open => if state == TxState::Closed { state = TxState::Open }, 
            }

            index += 1;
          }
          //Replace the dependencies.
          self.dependencies.set(Some(dependencies),);

          state
        };

        match state {
          //A dependency is holding this transaction open.
          TxState::Open => Err(TxError::OpenDependency),
          TxState::Aborted => {
            //Check if we meant to abort.
            if close {
              //A dependency is aborted while we meant to close.
              Err(TxError::AbortedDependency)
            } else {
              //We meant to abort.

              //Abort this transaction.
              self.tx_state.set(TxState::Aborted,);
              //Clear all dependencies.
              self.dependencies.set(Some(Vec::new()),);

              #[cfg(feature = "futures",)] {
              //Get all wakers.
              let notify = self.notify.replace(Some(Vec::new()),).into_iter()
                .flat_map(|notify,| notify,);
              //Notify all wakers.
              for waker in notify { waker.wake() }
              }
              #[cfg(feature = "old-futures",)] {
              //Get all tasks.
              let notify = self.notify.replace(Some(Vec::new()),).into_iter()
                .flat_map(|notify,| notify,);
              //Notify all wakers.
              for task in notify { task.notify() }
              }

              Ok(())
            }
          },
          //We are ready to close.
          TxState::Closed => {
            self.tx_state.set(TxState::Closed,);
            //Clear the dependencies.
            self.dependencies.set(Some(Vec::new()),);

            #[cfg(feature = "futures",)] {
            //Get all wakers.
            let notify = self.notify.replace(Some(Vec::new()),).into_iter()
              .flat_map(|notify,| notify,);
            //Notify all wakers.
            for waker in notify { waker.wake() }
            }
            #[cfg(feature = "old-futures",)] {
            //Get all tasks.
            let notify = self.notify.replace(Some(Vec::new()),).into_iter()
              .flat_map(|notify,| notify,);
            //Notify all wakers.
            for task in notify { task.notify() }
            }

            Ok(())
          },
          //A poisoned dependency immediatly exits while getting the `state`.
          TxState::Poisoned => unsafe { core::hint::unreachable_unchecked() },
        }
      },
    }
  }
  /// Returns the state of this transaction.
  #[inline]
  pub fn tx_state(&self,) -> TxState { self.tx_state.get() }
  /// Sets the transaction state to poisoned.
  pub fn poison(&self,) {
    self.tx_state.set(TxState::Poisoned,);

    #[cfg(feature = "futures",)] {
    //Get all wakers.
    let notify = self.notify.replace(Some(Vec::new()),).into_iter()
      .flat_map(|notify,| notify,);
    //Notify all wakers.
    for waker in notify { waker.wake() }
    }
    #[cfg(feature = "old-futures",)] {
    //Get all tasks.
    let notify = self.notify.replace(Some(Vec::new()),).into_iter()
      .flat_map(|notify,| notify,);
    //Notify all wakers.
    for task in notify { task.notify() }
    }
  }
  /// Clears the poisoned state of this transaction and resets it to opened.
  /// 
  /// This function has no effect when the transaction is not in a poisoned state.
  pub fn clear_poisoned(&self,) {
    if let TxState::Poisoned = self.tx_state.get() {
      self.tx_state.set(TxState::Open,)
    }
  }
  /// Returns `true` if `other_tx` is a dependency of `self` or if `other_tx` is `self`.
  pub fn will_wait(&self, other_tx: &Self,) -> bool {
    //Check if `other_tx` is self.
    if core::ptr::eq(self, other_tx,) { return true }

    //Get the dependencies.
    let dependencies = self.dependencies.replace(None,)
      .expect(DEPS_IN_USE);
    //Clone the dependencies.
    let deps = dependencies.iter()
      .map(Clone::clone,)
      .collect();
    //Replace the dependency list.
    self.dependencies.set(Some(dependencies),);

    //Check the dependency tree for `other_tx`.
    Self::_is_dependency(deps, other_tx,)
  }
  /// Adds `other_tx` as a dependency of this transaction.
  /// 
  /// Returns `false` if this transaction is already a dependency of `other_tx` or if 
  /// other_tx` is `self`.
  pub fn wait_for(&self, other_tx: &RcTxBox,) -> bool {
    //Check if this transaction is a dependecy of `other_tx`.
    if other_tx.will_wait(self,) { return false }
    
    //Add `other_tx` as a dependency.
    unsafe { self.wait_for_unchecked(other_tx,); }

    true
  }
  /// Adds `other_tx` as a dependency of this transaction.
  /// 
  /// # Safety
  /// 
  /// This function adds `other_tx` as a dependency without checking if the new
  /// dependency will create a cycle; it is therefor the responsibility of the caller to
  /// ensure no cycles will be created to avoid deadlocks.
  pub unsafe fn wait_for_unchecked(&self, other_tx: &RcTxBox,) {
    let mut dependencies = self.dependencies.replace(None,)
      .expect(DEPS_IN_USE);
    //Add `other_tx` as a dependency.
    dependencies.push(other_tx.clone(),);
    //Replace dependencies.
    self.dependencies.set(Some(dependencies),);
  }
  /// Adds `waker` to be notified once this transaction is resolved.
  #[cfg(feature = "futures",)]
  pub fn notify(&self, waker: Waker,) {
    let mut notify = self.notify.replace(None,)
      .expect(NOTIFY_IN_USE);
    //Add this `Waker` to be notified.
    notify.push(waker,);
    self.notify.set(Some(notify),);
  }
  /// Adds `task` to be notified once this transaction is resolved.
  #[cfg(feature = "old-futures",)]
  pub fn notify(&self, task: Task,) {
    let mut notify = self.notify.replace(None,)
      .expect(NOTIFY_IN_USE);
    //Add this `Task` to be notified.
    notify.push(task,);
    self.notify.set(Some(notify),);
  }
}

#[cfg(test,)]
mod tests {
  use super::*;

  #[test]
  fn test_tx_box() {
    //---test single transaction---
    let tx1 = Rc::new(TxBox::new(),);

    assert_eq!(tx1.close_tx(true,), Ok(()), "Error closing single transaction",);

    tx1.clear_poisoned();
    assert_eq!(tx1.tx_state(), TxState::Closed,
      "`clear_poisoned` had an effect when it should do nothing",
    );

    let tx1 = Rc::new(TxBox::new(),);

    tx1.poison();
    assert_eq!(tx1.tx_state(), TxState::Poisoned, "`poison` did not poison the transaction",);

    tx1.clear_poisoned();
    assert_eq!(tx1.tx_state(), TxState::Open,
      "`clear_poisoned` did nothing when it should clear the poisoned state",
    );

    //---test transactions with dependencies---

    let tx2 = Rc::new(TxBox::new(),);

    assert!(!tx1.will_wait(&tx2,), "`would_wait` `true` for non dependency",);
    assert!(tx1.wait_for(&tx2,), "Could not make `tx2` a dependency of `tx1`",);
    assert!(tx1.will_wait(&tx2,), "`would_wait` `false` for dependency",);
    assert!(!tx2.wait_for(&tx1,), "Could make `tx1` a dependency of `tx2`",);
    assert_eq!(tx1.close_tx(true,), Err(TxError::OpenDependency),
      "Closed `tx1` while `tx2` was still open",
    );
    assert_eq!(tx2.close_tx(true,), Ok(()), "Error closing `tx2`",);
    assert_eq!(tx1.close_tx(true,), Ok(()), "Error closing `tx1` after `tx2` was closed",);

    let tx1 = Rc::new(TxBox::new(),);
    let tx2 = Rc::new(TxBox::new(),);

    assert!(tx1.wait_for(&tx2,), "Could not make `tx2` a dependency of `tx1`",);
    assert_eq!(tx2.close_tx(false,), Ok(()), "Error aborting `tx2`",);
    assert_eq!(tx1.close_tx(true,), Err(TxError::AbortedDependency),
      "Closed `tx1` while `tx2` was aborted",
    );
    assert_eq!(tx1.close_tx(false,), Ok(()), "Error aborting `tx1` after `tx2` was aborted",);

    let tx1 = Rc::new(TxBox::new(),);
    let tx2 = Rc::new(TxBox::new(),);

    tx2.poison();
    tx1.wait_for(&tx2,);
    assert_eq!(tx1.close_tx(true,), Err(TxError::PoisonedError),
      "Closed `tx1` while `tx2` was poisoned",
    );
    tx1.clear_poisoned();
    assert_eq!(tx1.close_tx(true,), Ok(()),
      "Error closing `tx1` after clearing the poison state",
    );
    
    let tx1 = Rc::new(TxBox::new(),);
    let tx2 = Rc::new(TxBox::new(),);

    tx1.poison();
    tx1.wait_for(&tx2,);
    assert_eq!(tx1.close_tx(false,), Err(TxError::PoisonedError),
      "Aborted `tx1` while `tx2` was poisoned",
    );
  }
  #[test]
  fn test_tx_box_dependency_cleanup() {
    use core::mem;
    
    let tx1 = Rc::new(TxBox::new(),);
    let tx2 = Rc::new(TxBox::new(),);

    tx1.wait_for(&tx2,);
    let tx2_w = Rc::downgrade(&tx2,);

    mem::drop(tx2,);
    assert_eq!(tx1.close_tx(true,), Err(TxError::OpenDependency),
      "Closed `tx1` while `tx2` should still be open",
    );

    match tx2_w.upgrade() {
      None => panic!("`tx2` dropped while it was a dependeny of `tx1`"),
      Some(tx2) => if let Err(e) = tx2.close_tx(true,) {
        panic!("Error closing `tx2`: {:?}", e,)
      },
    }
    
    assert_eq!(tx1.close_tx(true,), Ok(()), "Error closing `tx1`",);
    assert!(tx2_w.upgrade().is_none(), "`tx1` did not clean its dependencies after closing",);
    
    let tx1 = Rc::new(TxBox::new(),);
    let tx2 = Rc::new(TxBox::new(),);

    tx1.wait_for(&tx2,);
    tx2.poison();
    let tx2_w = Rc::downgrade(&tx2,);
    
    mem::drop(tx2,);
    assert_eq!(tx1.close_tx(true,), Err(TxError::PoisonedError),
      "Closed `tx1` while `tx2` was poisoned",
    );

    assert!(tx2_w.upgrade().is_none(), "`tx1` did not remove `tx2` after poisoning",);
    
    tx1.clear_poisoned();
    assert_eq!(tx1.close_tx(true,), Ok(()), "Closed `tx1` after clearing the poisoned state",);
  }
  #[test]
  fn test_dependency_cycle_behaviour() {
    let tx1 = Rc::new(TxBox::new(),);

    assert!(tx1.will_wait(&tx1,), "`will_wait` is `false` for `self`",);

    let tx2 = Rc::new(TxBox::new(),);

    //Create cycle
    tx2.wait_for(&tx1,);
    unsafe { tx1.wait_for_unchecked(&tx2,); }

    assert_eq!(tx1.close_tx(true,), Err(TxError::OpenDependency), "Closed `tx1` while `tx2` was open",);
    assert_eq!(tx2.close_tx(true,), Err(TxError::OpenDependency), "Closed `tx2` while `tx1` was open",);
  }
}
