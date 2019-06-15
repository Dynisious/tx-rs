# tx-rs

A library providing `transactions` useful for implementing atomic or ordered
operations.

Transactions are a synchronisation primitive; a way of checkpointing the progress of
other threads of execution and/or enforcing an order of operations between threads.

```rust
use tx_rs::{TxState, sync::TxGuard};
use std::{thread, time::Duration};

let mut tx = TxGuard::new();
let tx_ref = tx.tx_ref();

thread::spawn(move || {
  thread::sleep(Duration::from_secs(2));
  println!("Finished first");
  tx.close().unwrap();
});

//Busy wait loop.
while tx_ref.tx_state() == TxState::Open {}

println!("Finished second");
```

Transaction references `TxRef` implement `Future` when compiled with the `futures`
or `old-futures` features; allowing them to be dropped into async tasks for
synchronisation.

An example using the `old-futures` feature:

```rust
use tx_rs::{TxState, sync::TxGuard};
use tokio::{prelude::*, timer::Delay};
use std::time::{Duration, Instant,};

let mut tx = TxGuard::new();
let tx_ref = tx.tx_ref();

tokio::run(future::lazy(move || {
  tokio::spawn(
    tx_ref.map(|_,| println!("Finished second"))
    .map_err(|_,| ())
  );

  tokio::spawn(
    Delay::new(Instant::now() + Duration::from_secs(2))
    .map(|_,| {
      println!("Finished first");
      tx.close().unwrap();
    })
    .map_err(|e,| eprintln!("{:?}", e))
  );
  
  future::ok(())
}));
```

Author --- daniel.bechaz@gmail.com  
Last Moddified --- 2019-06-15
