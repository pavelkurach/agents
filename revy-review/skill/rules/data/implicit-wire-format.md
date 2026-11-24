# implicit-wire-format
langs: any

When: Hand-encoded bytes written to or read from a socket, file, IPC or cache: struct packing, byte conversion, raw memory casts, a custom binary layout.
Ask: Is byte order fixed? Native order hides in Rust `to_ne_bytes`, `transmute` and `bytemuck` casts, Python `struct.pack` without a `<`, `>` or `!` prefix (native order and alignment), Go `binary.NativeEndian`, C struct dumps. Does an evolving payload carry a version and length, so old readers and new writers interoperate?
Not when: same-process memory only · a versioned codec (protobuf, a configured serde format) owns the layout.
Report as: `<encoder>` writes `<field>` in `<native order / raw layout>` with no version: another architecture or the next layout change misreads it silently; fix: fixed endianness, an explicit version and length prefix, or a schema'd codec.
