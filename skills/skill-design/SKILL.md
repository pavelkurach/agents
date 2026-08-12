---
name: skill-design
description: Write or tighten an agent skill so that deterministic work runs in shipped scripts and the prose carries only judgement. Use when creating a skill, reviewing one, or when a SKILL.md has grown long, repeats itself, or explains things the reader already knows.
---

# Skill design

A skill is read in full on every invocation. Words are a recurring cost; scripts are a one-time one.

## The reader is a capable model

**Delete anything explaining a general concept.** The reader knows type systems, LSP, git, migrations, HTTP, what a sequence diagram is for.

Keep only what is specific to this skill:

| Keep | Cut |
|---|---|
| Vocabulary the skill invented | Definitions of standard terms |
| Thresholds and their numbers | Why thresholds are useful |
| Tools, flags, exact invocations | What the tool category does |
| Rules a competent agent would get wrong | Rules a competent agent already follows |

**Test:** if a sentence would be true of any task in this domain anywhere, it is padding.

## Deterministic work goes in a script

**Never describe a procedure a script can perform.** Counting, cross-referencing, validating a format, deriving a graph, resolving symbols, collecting file metadata — all of it is a command, and the paragraph teaching the agent to do it by hand is dead weight.

| Prose smell | Replace with |
|---|---|
| "Check that every X has a Y" | a checker that exits non-zero |
| "Count the N and report it" | a script that prints the count |
| "Run these four probes and report" | one script, one call |
| A worked example of the output format | `--schema`, printed on demand |

Write the script. Delete the paragraph. Keep the one line saying which command and when.

## Ship the schema, not examples

**A schema plus a validator replaces every example.** Examples rot, drift from the validator, and cost words on every run.

```
uv run <tool>.py --schema          # the shape, printed on demand
uv run <tool>.py <file>            # deterministic feedback, precise errors
```

The error message is the teaching. `parts[2] has no 'value'` teaches more than a paragraph, arrives only when needed, and cannot disagree with the code.

**One declaration, many consumers.** The schema lives next to the code that consumes it. Docs reference it; they never restate it. Two copies of a shape means the doc is the copy nobody re-checks.

## Style

| Rule | |
|---|---|
| **Bold the rule, then stop** | Two sentences, not a paragraph |
| **Table for any enumeration** | Never a paragraph listing three things |
| **Show the command** | A fenced block beats a sentence describing one |
| **Three lines maximum per paragraph** | Break it or cut it |
| **Imperative, second person** | "Run X." not "The agent should run X." |
| **Prefer a period to an em dash** | Most em-dash clauses are bolted-on afterthoughts, and those are the cut |

Give a reason only where the rule is counterintuitive enough to be skipped without one. One clause. Never a paragraph of evidence.

## Every rule names the failure it prevents

**A rule with no failure behind it is a preference.** Cut it. When you cannot name what went wrong without it, you are guessing at what agents do.

Write the rule the shortest way that still stops the failure.

## Ship a checker for the skill's own docs

Rules point at things that must exist: a flag, a schema, a heading, a file. Those references rot silently.

**Write a script that validates the docs against the code.** A mandated format with no schema. A `§` reference to a heading nobody wrote. A flag the tool does not have. It runs in under a second with no model, and it catches the class of defect no review pass will.

## Calibrate every instrument

**An instrument's output is a claim, and claims get checked.** Before believing a number a script produced, give it a case whose answer you counted by hand.

A measuring script once reported perfect serialisation across 6,747 calls. It was reading the file format. The number reached a SKILL.md as a fact about behavior.

## Measure the instruction load

```
wc -w SKILL.md references/*.md
```

Run it before and after. Know the largest section in each file — that is what a read-on-demand rule should target, and it is usually the one that grew while nobody was looking.

## Checklist

- [ ] Every deterministic procedure is a script, not a paragraph
- [ ] Every format is a schema plus a validator, not an example
- [ ] Nothing explains a concept the reader already knows
- [ ] Every rule names the failure it prevents
- [ ] A checker validates the docs against the code, and passes
- [ ] Instruction load measured before and after
