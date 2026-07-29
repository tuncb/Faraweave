# Issue #33 — REPL terminal clearing

Use `std::io::IsTerminal` to keep redirected output byte-clean and emit ANSI clear-plus-home only for interactive terminals with a nonempty, non-`dumb` `TERM`. On Windows, use a narrowly isolated Console API FFI to clear the screen buffer and move the cursor home without adding a dependency or spawning a process. Unsupported capabilities and output or platform failures return explicit seam errors that the REPL reports before continuing.
Capability selection probes stdout with `GetConsoleScreenBufferInfo`; Windows PTYs that are terminals but not screen buffers use ANSI only when `TERM` permits it.
