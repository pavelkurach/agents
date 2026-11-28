# code

The code itself, read with the reader: what each part does, why it is written that way, and how the parts fit.
The reader leaves able to change it safely.

## Enumerate

| Enumerate | Scope |
|---|---|
| Subject | the file, module, type, or function asked about, at the explained revision |
| Structure | its types and functions, and how they relate — calls, ownership, data |
| Callers | every entry into it, one hop out |
| Callees | everything it calls out to, one hop out, and what each returns or raises |
| Data | what flows in, how it is transformed, what flows out; the state it reads or writes |
| Branches | every branch and error path, and the input that takes it |
| Tests | what exercises it, and the branches no test was found for |

## Body

1. **Placement** — a flowchart: callers → subject → callees, each edge a verb.
2. **Structure** — a diagram of its parts: types, functions, and the calls and data between them.
3. **Walkthrough** — the code in reading order, as whole units — a function, a type, an impl — split only at
   syntactic boundaries. The 20-line snippet bound does not apply here; notes per writing.md § Rules.
4. **Data through it** — a sequence or flowchart tracing one concrete input to its output.
5. **Branches** — a table: condition, an input that takes it, the result.
6. **Tests** — a table: test, what it covers; the branches where no test was found, with the search.
