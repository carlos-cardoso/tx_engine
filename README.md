# Readme

## Assumptions:
  - Only deposits can be disputed and then resolved/chargedback.
  - Once locked the account cannot process more transactions.
  - Values are rounded using bankers rounding to 4 decimal places at input/output

## Design patterns:

 - Since the toy problem consists of reading a csv input that is assumed to be in order, I skipped using async or parallelism since (except for locked accounts) we can only know the final state of the accounts once we go through the entire file.
 - In the case of locked accounts we can return the state of the account imediately and discard the transactions of that account.

 - Transaction Ids are globally unique so we can keep a single 

### Non async:

In a real setting with a web server, using tokio and async would be beneficial,
However, for this example it is not worth the added complexity since we are reading from a single csv file and writing to stdout.

### No CQRS (Command Query Resource Segregation) pattern:

Again this is overkill for the scope of this simple usecase. In a complex system decoupling the transaction commands from the read model would be beneficial.

### No Actor Frameworks (Kameo, Ractor etc.)

Since transavtions are sequential as they appear in the input file this negates the concurrent processing benefit of actors.

### Double Entry accounting to prevent errors?
