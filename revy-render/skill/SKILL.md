---
name: revy-render
description: Render a markdown plan or document into one polished, self-contained HTML page — highlighted code, linked file paths, colour-coded status, diagrams. Use when a skill hands you a `plan.md` (with an optional `presentation.md`) to render, or when the user asks to render a markdown file visually.
---

# revy-render

Present software-engineering information in the most effective form there is: every
item in the visual that makes it easiest to understand, colour that always means the same thing, nothing
reworded, nothing dropped. Judge each visual by one question — does it make this easier to understand than any
alternative? — and let that decide its kind and size.

Visuals, colour and interactivity: if it helps understanding, use it. A figure, a colour that carries meaning, a
hover, a link or a step-through that makes the content easier to grasp is always used; nothing is held back for
minimalism. The test stays the same — does it help understanding?

Inputs: the markdown file, an optional presentation file (how it should look), an output directory, and the
repository root(s) its paths refer to.

The content is the markdown, verbatim. You decide only the look, and the look has no ceiling: when a figure would
carry information better than the markdown's own shape, add it. The renderer handles markdown, code
highlighting, path links, status colours, and Mermaid on its own; [components.md](references/components.md) is
everything it can draw.

## Steps

### 1. Plan the look

Read the markdown and the presentation file. For each section, pick its best form per
[visuals.md](references/visuals.md); for each request in the presentation file, decide how to meet it. A request
to redraw an existing diagram is met by restyling it — kind classes, caption — keeping its nodes and edges.

**Done:** every section and every presentation request is mapped to a named form.

### 2. Look file

Write `look.json` in the output directory (fields in components.md § Look file): section order, folds, emphasis,
the list-tail fold, and `watermark` — `"DRAFT"` when the caller says the document is not final.

**Done:** every placement, fold, and emphasis the presentation file asks for is in `look.json`.

### 3. Figures

Copy the markdown to `render.md` in the output directory. Insert each figure from step 1 there, beside the
content it draws, as a fenced block from components.md, derived only from data already in the document. Change
nothing else in the copy.

**Done:** every node, edge, and number in a figure traces to the document.

### 4. Render

```
node <this skill>/render/render.mjs --plan <render.md> --source <original.md> --look <look.json> --out <dir> --repo <root> [--repo <alias>=<root>] [--link <web-url>@<sha>] [--evidence <log.md>]
node <this skill>/render/render.mjs --lint --plan <render.md> [--source <original.md>] [--look <look.json>] [--evidence <log.md>]
```

`--source` fails the render when the copy differs from the original anywhere outside the inserted figures.
Without figures, pass the original as `--plan` and drop `--source`. On first use run
`npm install --prefix <this skill>/render` (once, needs network).

**Done:** it printed `wrote <path>`, and every `warn:` line is fixed or reported.

### 5. Look at it

```
node <this skill>/render/check.mjs <dir>/<original name>.html --full <dir>/shots
```

It renders 1440, 1024, and 390 px in light and dark, prints one `✗` line per defect — page overflow, text under
12 px, text overflowing its shape or overlapping, clipped boxes, headings touching what follows — and writes a
contact sheet and full-page shots. Look at them too: a section in the wrong place, a figure that does not make
its claim. Fix and repeat, at most twice. Exit code 2 means no browser was found; say so. The last line is always
`✓ check passed` or `✗ check failed: N problems`; no such line means the check did not run.

**Done:** the check prints ✓, or each remaining problem is reported; the caller has the page path.
