# REPL exit command

After line-ending removal, REPL meta-commands match an exact case-sensitive spelling while borrowing the input with only surrounding ASCII spaces and tabs ignored. `.exit` returns success before source evaluation, and EOF retains the same successful result. Trailing source comments are not stripped for command matching, so `.exit # comment`, arguments, and prefixes remain ordinary source input.
