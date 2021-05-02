# Payment Engine test

Usage: `cargo run -- "tests/basic_deposits.csv"`  
There is an csv output (which may be empty) into stdout.  
There is also a logging output into stderr.

## Objective

Read transactions from csv format that will indicate deposits, withdrawals, etc, into clients accounts. The clients account balances are updated accordingly, and then the final state of their account balances are shown in csv format.

## Design

For the reading and writing on the csv format, including all field (column) values, the crates `serde` and `csv` are used. For the reading/writing of some (precision limited) decimal values, the `rust_decimal` is also used.

## Some Weaknesses

When the program is executed, all of the input is initially read into memory as a `Vec`, which is unnecessary because each input (transaction) is processed individually and in order.  
Thus reading and processing the inputs in a streaming fashion is desired (but is not currently implemented).  

There are two types of structures that are needed to be stored: clients balances and (some) past transactions. The later is needed because incoming transactions may refer to past ones.  
Both types were stored in their own `HashMap`, each having they own id as keys (that is, a client id for the client values, and transaction id for the transaction values).  

Therefore for each transaction that is being processed, only one client is potentially getting updates into their balances, which is a weakness if many transactions are incoming. For this engine test, one possible improvement is to potentially group transactions based on the client id (I believe that different clients can't interact with one another in this test), and thus it is unnecessary to "lock" all clients for each transaction that is being processed.  
The program currently hasn't been benchmarked, which is pretty much a necessity before trying speed-up designs.

## Current Workflolw

From how the data has be laid out, as new transactions are coming in, clients are consuming them. So the main processing logic resides in the clients - as it's their balances that get most of the state changes.

There is mostly one type of error, which is highly related to the clients as they are trying to consume transactions. Note: all errors related to clients consuming transactions are reported to stderr, but are ultimately ignored (the transaction is ignored).

Thinking on the error cases, which trigger early returns, I tried to avoid making state changes before all early returns - otherwise the states could be left partially changed.

The design in this project doesn't guarantee at compile-time that the contract mentioned above is followed. I tried making all state changes into the _copies_ of the states - not on the states themselves - and then tried to apply all actual changes on the states by state replacement, where all of them should be executed after all early-return points.

To help with this, there is the `apply` module, which includes, on it's traits, methods to `prepare` state changes (that happens on copies), and then those preparations can receive `apply`ments after all preparations and (hopefully) all early-return points.  
To (possibly) improve the chances of catching mistakes, I inserted a `Token` consumption when applying the prepared state changes, but the coders can create as many tokens as they want, so there is no guarantee.

## Tests

Currently only the most basic testing are included, which can be tried with `cargo test`.  
One improvement for correctness would be to capture all errors and excite every known error, at least from one path of execution.  
Another, would be to test the limits of the values, and verify if new errors should be considered as well.
