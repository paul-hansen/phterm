---
tags:
  - tuning
---
# `mux_output_parser_coalesce_delay_ms = 0`

How many milliseconds to wait after reading a chunk of output from a pane
before applying it to the terminal, in order to coalesce fragmented writes
into a single bigger chunk. A non-zero value can reduce the chance of
observing "screen tearing" with programs that don't use synchronized output,
at the cost of adding that much latency to every echoed keystroke.

Defaults to `0`. {{since('nightly', inline=True)}} Previously the default
was `3`. If you see partially drawn screens in full-screen terminal
programs, try `1`.

```lua
config.mux_output_parser_coalesce_delay_ms = 1
```
