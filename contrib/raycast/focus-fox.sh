#!/bin/bash

# Raycast script command that opens Focus Fox in a new Terminal window.
#
# Install: Raycast → Settings → Extensions → Script Commands → Add
# Directories, and add the folder containing this script (or copy it into
# your existing script directory). Then assign it a hotkey in Raycast if
# you want one.

# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Focus Fox
# @raycast.mode silent

# Optional parameters:
# @raycast.icon 🦊
# @raycast.packageName Focus Fox

# Documentation:
# @raycast.description Start Focus Fox in a new Terminal window
# @raycast.author Jordan Garrison
# @raycast.authorURL https://github.com/jordangarrison

osascript <<'EOF'
tell application "Terminal"
	do script "focus-fox"
	activate
end tell
EOF

# Prefer iTerm2? Replace the osascript block above with:
#
# osascript <<'EOF'
# tell application "iTerm"
# 	create window with default profile
# 	tell current session of current window to write text "focus-fox"
# 	activate
# end tell
# EOF
