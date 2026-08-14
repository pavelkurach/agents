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

# Progress bar helper: build_bar <percent_int> <width>
build_bar() {
  pct=$1
  width=$2
  filled=$(( (pct * width + 50) / 100 ))
  empty=$(( width - filled ))
  bar=""
  i=0
  while [ $i -lt $filled ]; do
    bar="${bar}█"
    i=$(( i + 1 ))
  done
  i=0
  while [ $i -lt $empty ]; do
    bar="${bar}░"
    i=$(( i + 1 ))
  done
  printf '%s' "$bar"
}

# Context used percentage
ctx_used=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
ctx_info=""
if [ -n "$ctx_used" ]; then
  ctx_int=$(printf '%.0f' "$ctx_used")
  ctx_bar=$(build_bar "$ctx_int" 8)
  ctx_info="\033[38;5;249mContext ${ctx_int}% ${ctx_bar}\033[0m"
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

# Strip ANSI escape codes to measure plain text length (BSD sed compatible)
ESC=$(printf '\033')
strip_ansi() {
  printf '%s' "$1" | sed "s/${ESC}\[[0-9;]*m//g"
}

# Join non-empty right-side segments with a grey " · " separator
sep="\033[38;5;240m ·\033[0m "
right_colored=""
for seg in "$quota_info" "$quota_7d_info" "$style_info" "$model_info" "$ctx_info"; do
  [ -z "$seg" ] && continue
  if [ -z "$right_colored" ]; then
    right_colored="$seg"
  else
    right_colored="${right_colored}${sep}${seg}"
  fi
done
[ -n "$right_colored" ] && right_colored="  ${right_colored}"

# Build left and right segments (with color codes)
left_colored="$(printf '\033[97m➜\033[0m  \033[97m%s\033[0m' "$dir")${git_info}"

# Measure visible lengths by stripping ANSI
left_plain=$(strip_ansi "$left_colored")
right_plain=$(strip_ansi "$right_colored")
left_len=${#left_plain}
right_len=${#right_plain}

# Get terminal width (fall back to 80)
term_width="${COLUMNS:-$(tput cols 2>/dev/null || echo 80)}"

# Calculate padding needed between left and right
pad=$(( term_width - left_len - right_len ))
if [ "$pad" -lt 1 ]; then pad=1; fi
padding=$(printf '%*s' "$pad" '')

printf '%b%s%b\n' "$left_colored" "$padding" "$right_colored"
