# ffi-ownership-crosses-boundary
langs: rust

When: Owned memory moved across FFI: `into_raw`/`from_raw`, `as_ptr` on an owned buffer, a pointer returned to C, or foreign memory freed.
Ask: For each `Box::into_raw`, `CString::into_raw` or returned pointer: which allocator created it, and which side frees it? Does a returned pointer outlive its owner (a stack local, a temporary `CString`, a dropped or reallocated `Vec`)? Is there exactly one free per allocation on every path?
Report as: `<pointer>` is `<freed by the other allocator / dangling / double-freed>`: heap corruption; fix: free on the allocating side through an exported `<name>_free`, and keep the owner alive while C holds the pointer.
