# untrusted-deserialization
langs: any

When: Bytes from a client, upload, queue, cache, shared bucket or downloaded artifact (model weights, plugin) deserialized or evaluated.
Ask: Can the format run code or build arbitrary objects — `pickle`, `cloudpickle`, `joblib`, `shelve`, `torch.load(weights_only=False)`, `numpy.load(allow_pickle=True)`, `yaml.load` with `Loader` or `UnsafeLoader`, `eval`/`exec`, `jsonpickle`, Java or .NET native serialization? Could anyone but this service have written the bytes? For length-prefixed binary formats (`bincode`, msgpack), is a size limit set before allocation?
Not when: `yaml.safe_load` · JSON parsed into typed models · `torch.load` with its default since PyTorch 2.6 (`weights_only=True`).
Report as: `<call>` deserializes `<source>` as `<format>`: whoever can write `<source>` runs code in this process `<or exhausts its memory>`; fix: a data-only format parsed into typed models with size limits, or sign trusted artifacts and verify before loading.
