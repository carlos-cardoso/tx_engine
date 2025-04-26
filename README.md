# Readme

## Assumptions:
  - Only deposits can be disputed and then resolved/chargedback.
  - Once locked the account cannot process more transactions.
  - Values are rounded using bankers rounding to 4 decimal places at input/output

## Design patterns:

 - Since the toy problem consists of reading a csv input that is assumed to be in order. 
   In the toy conditions there is not much possibility of gaining an advantage with async or parallelism since we can only know the final result of an account after we read the entire input unless it is locked/frozen (since there is no way to unclock).
   I used iterators to avoid loading the entire file in memory.
  
   If we were accepting parallel streams using a webserver, then it would make sense to use async (e.g. axum with tokio).
   If we had a database or keyvalue store it would make sense to use CQRS (Comand Query Resource Segregation) and Event Sourcing to separate the read side from the write side,
   The write side would commit the events pertaining to each specific client and then we could use materialized views to see the status of an account, the read side could either use pure event sourcing to materialize a view or use snapshotting to keep the client view updated depending on the usage characteristics.

 Since in the case of locked accounts we can return the state of the account imediately and discard the transactions of that account.
 I benchmarked adding a dedicated writer thread that would write the result imediately to the output:

 testing in hyperfine with a large csv (2.9GB):
wc -l testfile.csv
100000000 testfile.csv
ls -lh testfile.csv
-rw-r--r-- 1 carlos carlos 2,9G 26. Apr 21:39 testfile.csv

Tested on a Ryzen 9 laptop:

#### Read transactions and then write output:

Benchmark 1: cargo run --release -- /home/carlos/testfile.csv
  Time (mean ± σ):     52.683 s ±  1.262 s    [User: 51.784 s, System: 0.807 s]
  Range (min … max):   51.150 s … 55.401 s    10 runs

#### Dedicated writer thread with early write of locked accounts:

Benchmark 1: cargo run --release -- /home/carlos/testfile.csv
  Time (mean ± σ):     41.619 s ±  1.632 s    [User: 40.964 s, System: 0.816 s]
  Range (min … max):   39.627 s … 44.459 s    10 runs




