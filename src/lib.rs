//! A library providing `transactions` useful for implementing atomic or ordered
//! operations.
//! 
//! Transactions are a synchronisation primitive; a way of checkpointing the progress of
//! other threads of execution and/or enforcing an order of operations between threads.
//! 
//! ```rust
//! use tx_rs::{TxState, sync::TxGuard};
//! use std::{thread, time::Duration};
//! 
//! let mut tx = TxGuard::new();
//! let tx_ref = tx.tx_ref();
//! 
//! thread::spawn(move || {
//!   thread::sleep(Duration::from_secs(2));
//!   println!("Finished first");
//!   tx.close().unwrap();
//! });
//! 
//! //Busy wait loop.
//! while tx_ref.tx_state() == TxState::Open {}
//! 
//! println!("Finished second");
//! ```
//! 
//! Transaction references ([TxRef]) implement `Future` when compiled with the `futures`
//! or `old-futures` features; allowing them to be dropped into async tasks for
//! synchronisation.
//! 
//! An example using the `old-futures` feature:
//! 
//! ```ignore
//! use tx_rs::{TxState, sync::TxGuard};
//! use tokio::{prelude::*, timer::Delay};
//! use std::time::{Duration, Instant,};
//! 
//! let mut tx = TxGuard::new();
//! let tx_ref = tx.tx_ref();
//! 
//! tokio::run(future::lazy(move || {
//!   tokio::spawn(
//!     tx_ref.map(|_,| println!("Finished second"))
//!     .map_err(|_,| ())
//!   );
//! 
//!   tokio::spawn(
//!     Delay::new(Instant::now() + Duration::from_secs(2))
//!     .map(|_,| {
//!       println!("Finished first");
//!       tx.close().unwrap();
//!     })
//!     .map_err(|e,| eprintln!("{:?}", e))
//!   );
//!   
//!   future::ok(())
//! }));
//! ```
//! 
//! Author --- daniel.bechaz@gmail.com  
//! Last Moddified --- 2019-06-15

#![deny(missing_docs,)]
#![no_std]
#![feature(const_fn, const_vec_new, test, never_type,)]

#[cfg(all(feature = "futures", feature = "old-futures",),)]
compile_error!("`futures` and `old-futures` features are mutually exclusive",);

extern crate alloc;
#[cfg(test,)]
extern crate std;
#[cfg(test,)]
extern crate test;

mod tx_guard;
pub mod sync;

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
