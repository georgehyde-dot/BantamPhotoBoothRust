#!/bin/bash

# Kiosk startup script for Rust Photo Booth
LOG_FILE="/home/prospero/kiosk_$(date +%Y%m%d_%H%M%S).log"

echo "Starting Photo Booth Kiosk Mode at $(date)" >> "$LOG_FILE"

# Set display and kiosk mode flag
export DISPLAY=:0
export KIOSK_MODE=1
touch /tmp/kiosk_mode

# Disable keyring prompts
export GNOME_KEYRING_CONTROL=""
export GNOME_KEYRING_PID=""

echo "Environment variables set:" >> "$LOG_FILE"
echo "DISPLAY=$DISPLAY" >> "$LOG_FILE"
echo "KIOSK_MODE=$KIOSK_MODE" >> "$LOG_FILE"

# Disable screen blanking and power management (ignore errors)
xset s off >> "$LOG_FILE" 2>&1 || true
xset -dpms >> "$LOG_FILE" 2>&1 || true
xset s noblank >> "$LOG_FILE" 2>&1 || true

# Hide cursor after 1 second of inactivity
unclutter -idle 1 >> "$LOG_FILE" 2>&1 &

# Start virtual keyboard (hidden initially)
if command -v wvkbd-mobintl >/dev/null 2>&1; then
    echo "Starting virtual keyboard (hidden)..." >> "$LOG_FILE"
    wvkbd-mobintl -L 240 --hidden >> "$LOG_FILE" 2>&1 &
    KEYBOARD_PID=$!
    echo "Virtual keyboard started with PID: $KEYBOARD_PID" >> "$LOG_FILE"
fi

# Start the Rust application in background with explicit environment
echo "Starting Rust application with KIOSK_MODE=$KIOSK_MODE..." >> "$LOG_FILE"
cd /home/prospero
env KIOSK_MODE=1 ./rust_booth >> "$LOG_FILE" 2>&1 &
RUST_PID=$!

echo "Rust application started with PID: $RUST_PID" >> "$LOG_FILE"

# Wait for the server to start
echo "Waiting for server to start..." >> "$LOG_FILE"
sleep 5

# Check if server is responding
for i in {1..20}; do
    if curl -s http://localhost:8080 > /dev/null 2>&1; then
        echo "Server is responding after $i attempts" >> "$LOG_FILE"
        break
    fi
    echo "Attempt $i: Server not ready yet..." >> "$LOG_FILE"
    sleep 2
done

# Give it one more moment
sleep 2

# Create chromium temp directory
mkdir -p /tmp/chromium-kiosk

# Start Chromium in kiosk mode with keyring disabled
echo "Starting Chromium in kiosk mode..." >> "$LOG_FILE"
chromium-browser \
    --kiosk \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --disable-software-rasterizer \
    --disable-background-timer-throttling \
    --disable-renderer-backgrounding \
    --disable-backgrounding-occluded-windows \
    --disable-infobars \
    --disable-session-crashed-bubble \
    --disable-restore-session-state \
    --disable-features=TranslateUI,VizDisplayCompositor \
    --noerrdialogs \
    --disable-component-extensions-with-background-pages \
    --disable-extensions \
    --disable-field-trial-config \
    --force-device-scale-factor=1 \
    --start-fullscreen \
    --window-position=0,0 \
    --user-data-dir=/tmp/chromium-kiosk \
    --password-store=basic \
    --use-mock-keychain \
    --disable-password-generation \
    --disable-save-password-bubble \
    --app=http://localhost:8080 \
    >> "$LOG_FILE" 2>&1 &

BROWSER_PID=$!

echo "Kiosk started with Rust PID: $RUST_PID, Browser PID: $BROWSER_PID" >> "$LOG_FILE"

# Keep the script running and monitor processes
while true; do
    # Check if Rust app is still running
    if ! kill -0 $RUST_PID 2>/dev/null; then
        echo "Rust application died, restarting..." >> "$LOG_FILE"
        cd /home/prospero
        env KIOSK_MODE=1 ./rust_booth >> "$LOG_FILE" 2>&1 &
        RUST_PID=$!
        sleep 5
    fi
    
    # Check if browser is still running
    if ! kill -0 $BROWSER_PID 2>/dev/null; then
        echo "Browser died, restarting..." >> "$LOG_FILE"
        mkdir -p /tmp/chromium-kiosk
        chromium-browser \
            --kiosk \
            --no-sandbox \
            --disable-dev-shm-usage \
            --disable-gpu \
            --disable-software-rasterizer \
            --disable-background-timer-throttling \
            --disable-renderer-backgrounding \
            --disable-backgrounding-occluded-windows \
            --disable-infobars \
            --disable-session-crashed-bubble \
            --disable-restore-session-state \
            --disable-features=TranslateUI,VizDisplayCompositor \
            --noerrdialogs \
            --disable-component-extensions-with-background-pages \
            --disable-extensions \
            --disable-field-trial-config \
            --force-device-scale-factor=1 \
            --start-fullscreen \
            --window-position=0,0 \
            --user-data-dir=/tmp/chromium-kiosk \
            --password-store=basic \
            --use-mock-keychain \
            --disable-password-generation \
            --disable-save-password-bubble \
            --app=http://localhost:8080 \
            >> "$LOG_FILE" 2>&1 &
        BROWSER_PID=$!
        sleep 5
    fi
    
    # Check if keyboard died and restart if needed
    if [ -n "$KEYBOARD_PID" ] && ! kill -0 $KEYBOARD_PID 2>/dev/null; then
        if command -v wvkbd-mobintl >/dev/null 2>&1; then
            echo "Virtual keyboard died, restarting..." >> "$LOG_FILE"
            wvkbd-mobintl -L 240 --hidden >> "$LOG_FILE" 2>&1 &
            KEYBOARD_PID=$!
        fi
    fi
    
    sleep 10
done
