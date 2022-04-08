# Macro Language Specification 

## Statements 

Statements are always line delimited, that is, every line is a statement.

Statements that start with `>` followed by at least 1 space indicate that it is an instruction and thus will be interpreted.

Statements that do not start with `>` is assumed to be a minecraft command and sent to stdin after variable substitutions.

Example:
```
say hello
say world
```
(execute `say hello` and `say world` sequentially) 

```
> let a = 3
say hello
```
(set `a` equals to 3 and execute `say hello`)

`> say hello`
(error, `say` is not a valid instruction)

## Variables

Variables can be one of two types: `String` or `Int`.

Variables are declared using the `let` instruction.

Example:
```
> let a = 3
> let b = "hello"
```

Note the language is dynamically typed, so the following is allowed:
```
> let a = 3
> let a = "hello"
```

Variables can be dereferenced with `$` operator

Note that every time you want a value of a variable, you must dereference it
```
> let a = 4
say $a
```
(this will execute `say 4`)

Variables can be assigned to other variables:
```
> let a = 3
> let b = $a
```

## Environmental variables (not yet fully implemented)
`$0` first argument to the macro

`$1` second argument to the macro

`$x` xth argument to the macro (max 30)

`$31` end of the program

`$INSTANCE_NAME` name of the instance this macro belongs in

`$INSTANCE_UUID` name of the instance this macro belongs in

`$INSTANCE_PATH` path of the instance this macro belongs in


## Instructions

The language support some limited instruction, in a format similar to assembly languages 

## Arithmetic instructions:

`add var [op1] [op2]`

assigns the variable `var` to `op2 + op3`, declares the variable if it doesn't exist

Example:
```
> add $a 3 2
say $a
```
(executes `say 5`)

Note that the dereference op `$` is not neccessary for `var`, but it's neccessary for `op1` and `op2`.

The following is allowed:
```
> add a 3 2
```

But the following is an error:
```
> let c = 3
> add a c 2
```

`sub var op1 op2`

assigns the variable `var` to `op2 - op3`, declares the variable if it doesn't exist

`mult var op1 op2`

assigns the variable `var` to `op2 - op3`, declares the variable if it doesn't exist

`div var op1 op2`

assigns the variable `var` to `op2 / op3`, declares the variable if it doesn't exist

`mod var op1 op2`

assigns the variable `var` to `op2 % op3`, declares the variable if it doesn't exist

Note that `op1` and `op2` must all be int

## Branch instructions:

`goto [op]`

Transfer control to the `op`th line. (program starts at line 0)

Note that `op` must be an integer.

Example:
```
say loop
> goto 0
```
(executes `say loop` repeatedly, note repeated high frequency input like this will cause a server crash)

`beq [op1] [op2] [dest]`

Transfer control to `dest` if `op1 == op2`, supports string comparison.

`bne [op1] [op2] [dest]`

Transfer control to `dest` if `op1 != op2`, supports string comparison.

`bge [op1] [op2] [dest]`

Transfer control to `dest` if `op1 >= op2`, only integer comparison

`ble [op1] [op2] [dest]`

Transfer control to `dest` if `op1 <= op2`, only integer comparison

`bgt [op1] [op2] [dest]`

Transfer control to `dest` if `op1 > op2`, only integer comparison

`blt [op1] [op2] [dest]`

Transfer control to `dest` if `op1 < op2`, only integer comparison

`blt [op1] [op2] [dest]`

Transfer control to `dest` if `op1 < op2`, only integer comparison

`jalr [dest]` (jump and link register)

Store the next line's number into `$31`, transfer control to `dest`

Example:
```
> let ret = $31
> jalr procedure
say $3
> goto ret
> label procedure
> add $3 1 2
> goto $31
```

`label [name]`

Assigns `$name` to the line number of the label instruction.

Note that labels are not executed and automatically skipped

Example:

```
> let i = 3
> label loop
> ble i 0 $endloop 
say hi
> sub $i $i 1
> goto $loop
> label endloop
```
(execute `say hi` 3 times)

## Events

`> event <event name>`

Macro have the ability to listen to limited server events.

Macro will pause execution until the event happens.

Once the event is received, it may set some variables indicating the output of the event

`event player_joined`

Resume execution until a player has joined.

Set `$PLAYER_NAME` to the name of the player that just joined.

Example:
```
> event player_joined
say hello $PLAYER_NAME
> goto 0
```

`event player_left`

Resume execution until a player has left.

Set `$PLAYER_NAME` to the name of the player that just left.

`event player_chat`

Resume execution until a player sends a message

Set `$PLAYER_NAME` to the name of the player that just sent the message

Set `$CHAT_MSG` to the message of the player