#!/bin/sh
# Rebuilds fonts/*.woff2 from the installed JetBrains Mono Nerd Font (needs: pip install fonttools brotli).
# Usage: fonts/subset.sh [dir-with-JetBrainsMonoNerdFontMono-*.ttf]   (default ~/Library/Fonts)
# Subset: Latin, general punctuation, arrows, maths, box drawing, blocks, geometric shapes, dingbats.
# No Nerd icon (private-use) glyphs: the page uses none. Ligatures are dropped from the file (calt/liga/...)
# because code is set with ligatures off, which keeps the embedded files small.
set -eu
src=${1:-$HOME/Library/Fonts}
out=$(cd "$(dirname "$0")" && pwd)
for style in Regular Bold Italic; do
  pyftsubset "$src/JetBrainsMonoNerdFontMono-$style.ttf" \
    --unicodes='U+0020-007E,U+00A0-017F,U+2000-206F,U+2190-21FF,U+2200-22FF,U+2500-259F,U+25A0-25FF,U+2713-2717' \
    --layout-features='kern,tnum,zero' --flavor=woff2 --no-hinting --desubroutinize \
    --output-file="$out/JetBrainsMonoNerdFontMono-$style.subset.woff2"
done
