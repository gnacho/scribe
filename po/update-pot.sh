#!/bin/sh
# Regenerates po/scribe.pot from the sources listed in po/POTFILES and merges
# new messages into every po/*.po listed in po/LINGUAS.
# Requires: GNU gettext (xgettext, msgmerge, msgfmt).
set -e
cd "$(dirname "$0")/.."

xgettext --language=Rust --from-code=UTF-8 \
    --keyword=gettext --keyword=ngettext:1,2 \
    --package-name=scribe \
    --msgid-bugs-address=https://github.com/gnacho/scribe/issues \
    -o po/scribe.pot -f po/POTFILES

xgettext --language=Glade --keyword=translatable -j --from-code=UTF-8 \
    -o po/scribe.pot src/shortcuts.ui

cd po
while read -r lang; do
    [ -n "$lang" ] || continue
    msgmerge --update --previous "$lang.po" scribe.pot
done < LINGUAS
