# path-from-input-escapes-root
langs: any

When: Name from a request, archive entry or message joined onto a base directory, or used to read, write, serve or delete a file.
Ask: After `..`, symlinks and absolute parts resolve, is the result checked to lie under the resolved base? `os.path.join`, `pathlib`'s `/`, Rust `Path::join` and Node `path.resolve` discard the base when the input is absolute; `..` escapes every join. Archives: is each member checked on extraction (`tarfile` without `filter="data"`, the default only from Python 3.14; hand-rolled zip or tar loops)?
Not when: the name is a server-generated id or is matched against a fixed set.
Report as: `<path expr>` from `<source>` escapes `<base>` via `<.. / an absolute path / a symlink / an archive member>`: the caller can read or overwrite `<files>`; fix: resolve, then reject anything outside the resolved base (or open relative to a directory handle, Go `os.Root`), and store files under server-generated names.
