#!/bin/sh
input=$(cat)
# Debug: uncomment to capture input JSON shape
# printf '%s' "$input" > "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/statusline-debug.json"
cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // ""')
dir=$(basename "$cwd")

# Git branch + dirty marker (skip optional locks to avoid hangs)
git_info=""
if git_branch=$(GIT_OPTIONAL_LOCKS=0 git -C "$cwd" symbolic-ref --short HEAD 2>/dev/null); then
  dirty=""
  if [ -n "$(GIT_OPTIONAL_LOCKS=0 git -C "$cwd" status --porcelain 2>/dev/null)" ]; then
    dirty=" \033[38;5;203m✗\033[0m"
  fi
  git_info="  \033[38;5;250mgit:(${git_branch})\033[0m${dirty}"
fi

# Format a unix timestamp as time remaining until reset: "2h 15m" or "1d 3h"
format_reset() {
  ts=$1
  [ -z "$ts" ] && return
  now=$(date '+%s')
  delta=$(( ts - now ))
  [ "$delta" -lt 0 ] && delta=0
  days=$(( delta / 86400 ))
  hours=$(( (delta % 86400) / 3600 ))
  minutes=$(( (delta % 3600) / 60 ))
  if [ "$days" -gt 0 ]; then
    printf '%dd %dh' "$days" "$hours"
  elif [ "$hours" -gt 0 ]; then
    printf '%dh %dm' "$hours" "$minutes"
  else
    printf '%dm' "$minutes"
  fi
}

# Format a unix timestamp as an absolute reset time: "15:00" (today) or "Mon 15:00" (other day)
format_reset_absolute() {
  ts=$1
  [ -z "$ts" ] && return
  today=$(date '+%Y-%m-%d')
  reset_day=$(date -r "$ts" '+%Y-%m-%d' 2>/dev/null)
  if [ "$reset_day" = "$today" ]; then
    date -r "$ts" '+%H:%M' 2>/dev/null
  else
    day_abbr=$(date -r "$ts" '+%a' 2>/dev/null)
    time_str=$(date -r "$ts" '+%H:%M' 2>/dev/null)
    printf '%s %s' "$day_abbr" "$time_str"
  fi
}

# 5hr rate-limit quota used
quota_5h=$(echo "$input" | jq -r '.rate_limits.five_hour.used_percentage // empty')
quota_5h_reset=$(echo "$input" | jq -r '.rate_limits.five_hour.resets_at // empty')
quota_info=""
if [ -n "$quota_5h" ]; then
  quota_int=$(printf '%.0f' "$quota_5h")
  # color escalates: green <50, yellow 50-79, red >=80
  if [ "$quota_int" -ge 80 ]; then
    qcolor=203
  elif [ "$quota_int" -ge 50 ]; then
    qcolor=221
  else
    qcolor=151
  fi
  reset_str=$(format_reset "$quota_5h_reset")
  [ -n "$reset_str" ] && reset_str=" (${reset_str})"
  quota_info="\033[38;5;${qcolor}m5-hour ${quota_int}%${reset_str}\033[0m"
fi

# 7-day rate-limit quota used (stricter thresholds — weekly is harder to recover)
quota_7d=$(echo "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')
quota_7d_reset=$(echo "$input" | jq -r '.rate_limits.seven_day.resets_at // empty')
quota_7d_info=""
if [ -n "$quota_7d" ]; then
  quota_7d_int=$(printf '%.0f' "$quota_7d")
  # color escalates: green <40, yellow 40-69, red >=70
  if [ "$quota_7d_int" -ge 70 ]; then
    q7dcolor=203
  elif [ "$quota_7d_int" -ge 40 ]; then
    q7dcolor=221
  else
    q7dcolor=151
  fi
  reset_str=$(format_reset_absolute "$quota_7d_reset")
  [ -n "$reset_str" ] && reset_str=" (${reset_str})"
  quota_7d_info="\033[38;5;${q7dcolor}mWeekly ${quota_7d_int}%${reset_str}\033[0m"
fi

# Model display name
model=$(echo "$input" | jq -r '.model.display_name // ""')

# Reasoning effort level
effort=$(echo "$input" | jq -r '.effort.level // ""')

# Context used percentage
ctx_used=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
ctx_info=""
if [ -n "$ctx_used" ]; then
  ctx_int=$(printf '%.0f' "$ctx_used")
  # color escalates: green <40, yellow 40-69, red >=70
  if [ "$ctx_int" -ge 70 ]; then
    ctxcolor=203
  elif [ "$ctx_int" -ge 40 ]; then
    ctxcolor=221
  else
    ctxcolor=151
  fi
  ctx_info="\033[38;5;${ctxcolor}mContext ${ctx_int}%\033[0m"
fi

# Output style (only shown when non-default)
output_style=$(echo "$input" | jq -r '.output_style.name // ""')
style_info=""
if [ -n "$output_style" ] && [ "$output_style" != "default" ]; then
  style_info="\033[38;5;180m${output_style}\033[0m"
fi

# Model info (with effort level, if present)
model_info=""
if [ -n "$model" ]; then
  if [ -n "$effort" ]; then
    model_info="\033[38;5;230m${model} ${effort}\033[0m"
  else
    model_info="\033[38;5;230m${model}\033[0m"
  fi
fi

# Join non-empty segments with a grey " · " separator
sep="\033[38;5;240m ·\033[0m "
join_segments() {
  joined=""
  for seg in "$@"; do
    [ -z "$seg" ] && continue
    if [ -z "$joined" ]; then
      joined="$seg"
    else
      joined="${joined}${sep}${seg}"
    fi
  done
  printf '%s' "$joined"
}

# Strip literal "\033[...m" escape sequences (not yet interpreted — that happens at printf %b time)
# to measure plain text length.
strip_ansi() {
  printf '%s' "$1" | sed 's/\\033\[[0-9;]*m//g'
}

# Line 1 base: dir/git + model + context + style; quotas join it too if they fit, else wrap to line 2
line1_base=$(join_segments "$model_info" "$ctx_info" "$style_info")
quotas=$(join_segments "$quota_info" "$quota_7d_info")

# Build left segment (with color codes)
left_colored="\033[97m➜\033[0m  \033[97m${dir}\033[0m${git_info}"

line1=$(join_segments "$left_colored" "$line1_base")

# Claude Code sets COLUMNS to the real terminal width before running this script
# (no pty is attached, so tput cols won't work here); fall back to 80 if unset.
term_width="${COLUMNS:-80}"

if [ -n "$quotas" ]; then
  line1_with_quotas=$(join_segments "$line1" "$quotas")
  line1_with_quotas_plain=$(strip_ansi "$line1_with_quotas")
  line1_with_quotas_len=${#line1_with_quotas_plain}
  if [ "$line1_with_quotas_len" -le "$term_width" ]; then
    printf '%b\n' "$line1_with_quotas"
  else
    printf '%b\n' "$line1"
    printf '%b\n' "$quotas"
  fi
else
  printf '%b\n' "$line1"
fi
