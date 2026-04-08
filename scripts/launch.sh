#!/usr/bin/env bash
# HumanityOS Launcher — works on Linux, macOS, Raspberry Pi, any POSIX system
# Finds and runs the latest versioned binary in the binaries directory.

BINDIR="${HUMANITYOS_BINDIR:-$HOME/Humanity}"

if [ ! -d "$BINDIR" ]; then
    echo "ERROR: $BINDIR does not exist."
    echo "Run 'just build-game' first, or set HUMANITYOS_BINDIR."
    exit 1
fi

# Find the most recently modified binary matching v*_HumanityOS*
LATEST=$(ls -t "$BINDIR"/v*_HumanityOS* 2>/dev/null | head -1)

if [ -z "$LATEST" ]; then
    echo "ERROR: No HumanityOS builds found in $BINDIR"
    echo "Run 'just build-game' first."
    exit 1
fi

echo "Launching $(basename "$LATEST")"
exec "$LATEST"
