# slice-aliases-backing-array
langs: go

When: Append to a slice the code did not allocate (parameter, field, `s[:n]`), a stored or returned sub-slice, or bytes kept from a reused buffer.
Ask: Do two slices share a backing array? `append` to `a[:n]` overwrites `a[n]` when capacity allows; appends to a caller's slice may or may not reach the caller; `Scanner.Bytes()`, `Buffer.Bytes()` and pooled buffers change on the next use; Go 1.22+ `slices.Delete` and `Compact` zero the vacated tail that other aliases still see. A small sub-slice pins its whole buffer.
Report as: `<slice>` shares the backing array of `<other>`: `<append overwrites its element / stored bytes change after the next read / the large buffer is retained>`; fix: copy (`slices.Clone`, `bytes.Clone`), or cap it with `s[:n:n]` before appending.
Example:
  bad:  base := s[:2]; a := append(base, x); b := append(base, y)   // a[2] == y
  good: base := s[:2:2]
