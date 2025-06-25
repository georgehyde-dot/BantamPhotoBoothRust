#!/bin/bash

# Enhanced keyboard toggle script with better debugging
LOG_FILE="/home/prospero/keyboard_debug.log"

echo "$(date): Toggle keyboard called" >> "$LOG_FILE"

# Ensure DISPLAY is set
export DISPLAY=:0

# Check if keyboard is running
KEYBOARD_PID=$(pgrep -f wvkbd-mobintl)
echo "$(date): Current keyboard PID: $KEYBOARD_PID" >> "$LOG_FILE"

if [ -n "$KEYBOARD_PID" ]; then
    # Keyboard is running, kill it
    echo "$(date): Killing keyboard PID $KEYBOARD_PID" >> "$LOG_FILE"
    kill $KEYBOARD_PID
    sleep 0.5
    echo "$(date): Keyboard hidden" >> "$LOG_FILE"
    echo "hidden"
else
    # Start keyboard
    echo "$(date): Starting keyboard..." >> "$LOG_FILE"
    
    # Try to start keyboard and capture any errors
    wvkbd-mobintl -L 300 >> "$LOG_FILE" 2>&1 &
    NEW_PID=$!
    
    echo "$(date): Started keyboard with PID $NEW_PID" >> "$LOG_FILE"
    
    # Give it a moment to start
    sleep 1
    
    # Check if it's actually running
    if kill -0 $NEW_PID 2>/dev/null; then
        echo "$(date): Keyboard successfully started" >> "$LOG_FILE"
        echo "shown"
    else
        echo "$(date): Keyboard failed to start" >> "$LOG_FILE"
        echo "failed"
    fi
fi