# ffi-panic-crosses-boundary
langs: rust

When: Code that can panic added inside an `extern "C"` function or a callback called from C.
Ask: Can the body panic? Since Rust 1.81 a panic leaving `extern "C"` aborts the process (earlier it was undefined behaviour); `extern "C-unwind"` unwinds into foreign frames.
Report as: a panic in `<fn>` escapes to C: the host process aborts instead of getting an error code; fix: `catch_unwind` around the body, mapping a panic to an error return.
