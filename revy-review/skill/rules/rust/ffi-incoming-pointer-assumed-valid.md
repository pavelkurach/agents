# ffi-incoming-pointer-assumed-valid
langs: rust

When: Exported `extern "C"` function or C callback with pointer parameters added or changed.
Ask: Per the C contract, may each pointer be null, uninitialized, misaligned or short-lived? Typing it `&T`/`&mut T`, reading through it, or `*p = v` (which drops the old value) assumes valid initialized memory; out-params usually point at uninitialized storage.
Not when: the contract requires an initialized in/out value.
Report as: `<fn>` treats `<param>` as `<non-null / initialized>` without a caller guarantee: undefined behaviour; fix: a raw pointer, a null check returning an error code, `ptr::write` or `MaybeUninit` for out-params.
Example:
  bad:  pub extern "C" fn load(out: &mut Config) -> i32 { *out = read(); 0 }
  good: pub unsafe extern "C" fn load(out: *mut Config) -> i32 { if out.is_null() { return -1 } out.write(read()); 0 }
