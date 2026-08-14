# Claude Code status line

Status line for [Claude Code](https://claude.com/claude-code). Shows working dir, git branch/dirty marker, the 5-hour and weekly rate-limit quotas (with reset times), model, context-window usage, and output style (if non-default). Model/context/style join the first line if there's room, otherwise wrap onto a second line.

```
➜  myrepo  git:(main) ✗ · 5-hour 14% (2h 15m) · Weekly 40% (Mon 09:00) · Opus 4.7 high · Context 42% · explanatory
```

On a narrow terminal:

```
➜  myrepo  git:(main) ✗ · 5-hour 14% (2h 15m) · Weekly 40% (Mon 09:00)
Opus 4.7 high · Context 42% · explanatory
```

Segments (left → right):

| Segment | Source | Notes |
|---|---|---|
| `➜ <dir>` | `$cwd` basename | white arrow + dir |
| `git:(<branch>)` | `git symbolic-ref` | omitted if not a git repo |
| `✗` | `git status --porcelain` | red, present only if working tree dirty |
| `5-hour N% (...)` | `rate_limits.five_hour.{used_percentage,resets_at}` | grey <50, amber 50–79, red ≥80; time remaining until reset, e.g. `2h 15m`, `1d 3h`, or `40m` |
| `Weekly N% (...)` | `rate_limits.seven_day.{used_percentage,resets_at}` | grey <50, amber 50–79, red ≥80; absolute reset time (24h), time-only if today else `Day HH:MM` |
| `<style>` | `output_style.name` | mauve |
| `<model>` | `model.display_name` | warm cream |
| `Context N%` | `context_window.used_percentage` | grey <40, dark amber 40–69, dark red ≥70 — darker variants of the quota palette |

## Requires

- `jq` — Claude Code pipes JSON to stdin
- POSIX `sh`, `sed`, `date`, `git` (default on macOS/Linux)
- Claude Code v2.1.153+ (sets `COLUMNS`/`LINES` before running the status line command)

## Install

1. Copy the script and wire it in:
   ```sh
   mkdir -p ~/.claude
   cp statusline-command.sh ~/.claude/statusline-command.sh
   chmod +x ~/.claude/statusline-command.sh
   ```

2. Register it in `~/.claude/settings.json`:
   ```json
   {
     "statusLine": {
       "type": "command",
       "command": "bash ~/.claude/statusline-command.sh"
     }
   }
   ```

3. Restart Claude Code (or wait one tick — Claude re-invokes the command on every render).

### Tracked-repo variant (recommended)

Keep the script under version control in this repo, then symlink it into `~/.claude` so edits propagate without copying:

```sh
ln -sf "$(pwd)/statusline-command.sh" ~/.claude/statusline-command.sh
```

If `~/.claude/statusline-command.sh` already exists as a real file, back it up first:

```sh
mv ~/.claude/statusline-command.sh ~/.claude/statusline-command.sh.bak
ln -sf "$(pwd)/statusline-command.sh" ~/.claude/statusline-command.sh
```

## Debugging

Need to see the raw JSON Claude Code pipes to the statusline? Uncomment line 4:

```sh
printf '%s' "$input" > "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/statusline-debug.json"
```

Wait one statusline tick, then `cat ~/.claude/statusline-debug.json | jq .`.

Test the script locally against captured JSON:

```sh
cat ~/.claude/statusline-debug.json | sh ~/.claude/statusline-command.sh
```

## Customize

- **Colors**: 256-color ANSI codes inline (`\033[38;5;NNNm`).
- **Segment separator**: segments join with a grey `" · "`; edit the `sep` variable.
- **Quota thresholds**: edit `if [ "$quota_int" -ge 80 ]` / `-ge 50` blocks.
- **Fallback terminal width**: `${COLUMNS:-80}`, used only to decide whether model/context/style wrap to line 2.
