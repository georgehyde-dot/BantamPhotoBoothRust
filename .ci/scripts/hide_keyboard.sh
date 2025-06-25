#!/bin/bash
LOG_FILE="/home/prospero/keyboard_debug.log"

echo "$(date): Hide keyboard called" >> "$LOG_FILE"

# Create lock to prevent race conditions
LOCK_FILE="/tmp/keyboard_hide.lock"
if ! mkdir "$LOCK_FILE" 2>/dev/null; then
    echo "$(date): Hide operation already in progress" >> "$LOG_FILE"
    exit 0
fi

cleanup() {
    rmdir "$LOCK_FILE" 2>/dev/null || true
}
trap cleanup EXIT

KEYBOARD_PIDS=$(pgrep -f wvkbd-mobintl)
if [ -n "$KEYBOARD_PIDS" ]; then
    echo "$(date): Killing keyboard PIDs: $KEYBOARD_PIDS" >> "$LOG_FILE"
    pkill -f wvkbd-mobintl
    
    # Wait for clean shutdown
    sleep 0.5
    
    # Force kill if needed
    if pgrep -f wvkbd-mobintl > /dev/null; then
        pkill -9 -f wvkbd-mobintl
    fi
    
    echo "$(date): Keyboard hidden" >> "$LOG_FILE"
else
    echo "$(date): No keyboard running to hide" >> "$LOG_FILE"
fi
