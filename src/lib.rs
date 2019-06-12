//! A small library providing transactions useful for implementing atomic or ordered
//! operations.
//! 
//! Author --- daniel.bechaz@gmail.com  
//! Last Moddified --- 2019-06-12

#![deny(missing_docs,)]
#![no_std]
#![feature(cell_update, range_is_empty, weak_counts, const_fn,)]

extern crate alloc;
#[cfg(test,)]
extern crate std;

mod tx_guard;

pub use self::tx_guard::*;

/// The possible states of a transaction.
#[derive(PartialEq, Eq, Clone, Copy, Debug,)]
pub enum TxState {
  /// The transaction is still open.
  Open,
  /// The thread holding the transaction open paniced.
  Poisoned,
  /// The transaction has been aborted.
  Aborted,
  /// The transaction has been closed.
  Closed,
}

/// The error type returned by this crate.
#[derive(PartialEq, Eq, Clone, Debug,)]
pub enum TxError {
  /// Attempted to close a transaction which was already closed or aborted.
  AlreadyClosed,
  /// A dependency of a transaction is still open.
  OpenDependency,
  /// A dependency of a transaction aborted.
  AbortedDependency,
  /// A transaction is in a poisoned state.
  /// 
  /// One cause for this is when a thread panics whole holding an open transaction.
  PoisonedError,
}

/// The result type returned by this crate.
pub type TxResult<T> = Result<T, TxError>;
