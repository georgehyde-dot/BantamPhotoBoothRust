#!/bin/bash
export DISPLAY=:0
LOG_FILE="/home/prospero/keyboard_debug.log"

echo "$(date): Show keyboard called" >> "$LOG_FILE"

# Check if keyboard is already running
KEYBOARD_PID=$(pgrep -f wvkbd-mobintl)

if [ -z "$KEYBOARD_PID" ]; then
    echo "$(date): Starting keyboard..." >> "$LOG_FILE"
    wvkbd-mobintl -L 300 >> "$LOG_FILE" 2>&1 &
    echo "$(date): Keyboard started" >> "$LOG_FILE"
else
    echo "$(date): Keyboard already running with PID $KEYBOARD_PID" >> "$LOG_FILE"
fi