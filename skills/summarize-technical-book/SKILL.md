---
name: summarize-technical-book
description: Use when a user provides a technical non-fiction PDF or EPUB and wants a durable chapter-by-chapter engineering summary, key learnings, or critical review.
---

# Summarize technical book

Extract lessons that improve future engineering decisions. Chapter coverage supports that goal; it is not the goal.

Run commands from this skill directory. Use `uv`; only PDF preparation needs the `pypdf` dependency.

## Output judgment

| Section | Include | Boundary |
|---|---|---|
| Summary | Chapter's argument and evidence needed to understand its lessons | Concise context, not a replay |
| Learnings | Advice that could change design, implementation, debugging, review, reliability, or operations | Bulleted paragraphs; bold takeaway first, then mechanism, applicability, trade-off, or consequence |
| Key Concepts | Shared field vocabulary that transfers beyond one implementation and explains or predicts behavior | Exclude products, libraries, standards, symbols, commands, configuration, incidents, numbers, and book-specific examples; omit the heading when empty |
| Technologies & APIs | Concrete tools or interfaces the reader may deliberately apply later | Selective; group related symbols; omit the heading when empty |
| Critical Commentary | Corrections, dated claims, missing context, and further reading | Label added empirical claims `[verified: URL]` or `[model-claim]` |

Use incidents, numbers, and examples only as evidence for a Summary or Learning. A section stops when no remaining material passes its inclusion rule. Never fill a quota.

Use this concept test: could a knowledgeable practitioner explain the idea without referring to this book or a particular package? Observability and leaderless replication can qualify. `tokio::sync::watch`, `reqwest`, database settings, and command-line flags belong under Technologies & APIs when they are worth retaining.

Group Learnings with `####` headings only when at least two coherent subtopics each contain multiple learnings. Fence multi-line code with a language tag.

Print the canonical structure whenever a writer needs it:

```bash
uv run scripts/summary_contract.py schema
```

## Workflow

### 1. Prepare the book

```bash
uv run --with pypdf scripts/prepare_book.py /absolute/path/book.pdf
uv run scripts/prepare_book.py /absolute/path/book.epub
```

The command prints the path to a stable work-directory manifest. It extracts chapter text, reuses matching work on rerun, and reports when a PDF needs OCR. Use `--work-dir PATH` to choose the workspace or `--force` to replace that book's generated workspace.

Read `manifest.json`. Confirm the final output directory, included sections, and treatment of appendices or closing material before spending model calls. For 30 or more short sections, offer coherent grouping.

### 2. Build concept evidence

Print the input schema:

```bash
uv run scripts/concept_inventory.py schema
```

For each selected `chapters/chNN.txt`, write `concepts/chNN.json`. Apply the Key Concepts boundary before emitting an entry. `introduced` means the chapter establishes the concept; `elaborated` means it materially deepens it. Mere mentions produce no entry.

Process chapters concurrently when independent workers are available; otherwise process them sequentially. Then validate and group the evidence:

```bash
uv run scripts/concept_inventory.py collate WORK/concepts WORK/concept-evidence.json
```

Read the grouped evidence and write `WORK/concepts.md`: one concise, cross-chapter definition per surviving concept. The script groups evidence; synthesizing the definition remains a judgment task.

### 3. Calibrate one chapter

Draft the first included chapter using its extracted text, `concepts.md`, and the Output judgment table. Save it as `WORK/fragments/chNN.md`, then run:

```bash
uv run scripts/summary_contract.py check-fragment WORK/fragments/chNN.md
```

Show the draft to the user. Ask specifically about Summary depth, Learning quality, Concept classification, and Technologies/API selection. Record accepted feedback in `WORK/style-notes.md` and redraft until approved.

### 4. Draft the remaining chapters

Give each writer only its chapter text, `concepts.md`, `style-notes.md`, the Output judgment table, and the schema command's structure. Preserve source claims; do not invent lessons to make a section look substantial.

Validate every returned fragment. On failure, return the validator errors to that writer once. If the retry still fails, stop and surface the chapter and errors.

### 5. Synthesize and assemble

Write `WORK/book-wide.md` beginning with `## Book-wide Learnings`. Deduplicate the book's highest-value engineering lessons in the same bulleted-paragraph form. Preserve applicability and trade-offs. Material the book did not teach belongs in Critical Commentary, not this synthesis.

Assemble in one deterministic step:

```bash
uv run scripts/summary_contract.py assemble \
  --title "BOOK TITLE" \
  --author "AUTHOR" \
  --chapters-dir WORK/fragments \
  --book-wide WORK/book-wide.md \
  --output "/absolute/output/BOOK TITLE.md"
```

Add `--preamble WORK/preamble.md` only for substantive front matter the user chose to retain. Assembly validates before writing. Print the final path and keep the work directory for resume until the user no longer needs it.
Use `--force` only when the user has approved replacing an existing final output.

## Resume and revision

The manifest, inventories, style notes, and fragments are the progress record. Reuse valid files and regenerate only missing or rejected stages. Preparation's `--force` starts extraction over; assembly's `--force` replaces the final output.

When the user requests a uniform style or section change, regenerate every affected fragment and reassemble. When content is merely missing from one chapter, regenerate only that chapter.

## Failure boundaries

- Reject fiction and memoir unless the user explicitly accepts this technical format.
- Preserve the extraction warning when PDF structure was inferred rather than read from an outline.
- Treat malformed worker JSON or Markdown as invalid output, never as best-effort content.
- Keep external commentary visibly separate from the book's teachings.

Before publishing changes to this skill, run:

```bash
python3 -m unittest discover tests -v
```
