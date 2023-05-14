# Plan

1. Create a C wrapper around `perf_event_open()` and reading the event buffer as
   raw bytes.
1. Rust module to create a stream of samples - async.
1. Store samples as JSON.
1. Sample processing - flamegraph, timeline, time list.
1. Try `addr2line` debug symbols.
1. Complete sampling implementation, output is JSON data.
1. Provide proper JSON API.
   - Initiate sampling with options
     - CPU-wide vs process-only
     - resolve PID to process name
     - catch process arguments
     - resolve symbols
     - callstack
     - sampling rate
     - sampling time
   - Stop sampling

# TUI

1. Use crossterm
1. Use 2 buffers, print diff.
1. Event-based design.
1. Resolve ownership, decoupling of UI elements and logic.
1. Timeline, flamegraph, list views.
