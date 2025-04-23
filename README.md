# Readme


## Design pattern decisions:

### Non async:

In a real setting with a web server, using tokio and async would be beneficial,
However, for this example it is not worth the added complexity since we are reading from a single csv file and writing to stdout.

### No CQRS (Command Query Resource Segregation) pattern:

Again this is overkill for the scope of this simple usecase. In a complex system decoupling the transaction commands from the read model would be beneficial.

### No Actor Frameworks (Kameo, Ractor etc.)

Since transavtions are sequential as they appear in the input file this negates the concurrent processing benefit of actors.

### Double Entry accounting to prevent errors?
