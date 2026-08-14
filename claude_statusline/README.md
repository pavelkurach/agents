# Claude Code status line

Status line for [Claude Code](https://claude.com/claude-code). Shows working dir, git branch + dirty marker, 5-hour and weekly rate-limit quotas (with reset times), output style (if non-default), model, and context-window usage.

```
➜  myrepo  git:(main) ✗   5-hour 14% (2h 15m) · Weekly 40% (5d 8h) · explanatory · Opus 4.7 high · Context 42%
```

Segments (left → right):

| Segment | Source | Notes |
|---|---|---|
| `➜ <dir>` | `$cwd` basename | white arrow + dir |
| `git:(<branch>)` | `git symbolic-ref` | omitted if not a git repo |
| `✗` | `git status --porcelain` | red, present only if working tree dirty |
| `5-hour N% (...)` | `rate_limits.five_hour.{used_percentage,resets_at}` | green <50, yellow 50–79, red ≥80; time remaining until reset, e.g. `2h 15m`, `1d 3h`, or `40m` |
| `Weekly N% (...)` | `rate_limits.seven_day.{used_percentage,resets_at}` | green <40, yellow 40–69, red ≥70; same reset format |
| `<style>` | `output_style.name` | omitted when style is `default` |
| `<model>` | `model.display_name` | cream-coloured |
| `Context N%` | `context_window.used_percentage` | percent of context window used |

## Requires

- `jq` — Claude Code pipes JSON to stdin
- POSIX `sh`, `sed`, `tput`, `date`, `git` (default on macOS/Linux)

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
- **Segment separator**: right-side segments join with a grey `" · "`; edit the `sep` variable.
- **Quota thresholds**: edit `if [ "$quota_int" -ge 80 ]` / `-ge 50` blocks.
- **Fallback terminal width**: `${COLUMNS:-$(tput cols ... || echo 80)}`.
