#!/bin/bash

set -e

# --- Configuration ---
PI_USER="prospero"
PI_HOST="BantamPhotoShop.local"
BINARY_NAME="rust_photobooth"
REMOTE_DEST_PATH="/home/${PI_USER}/rust_booth"
DOCKER_IMAGE_NAME="pi-builder"
SSH_KEY_PATH="$HOME/.ssh/id_bantatam_pi"
LOG_FILE="rust_booth_$(date +%Y%m%d_%H%M%S).log"

echo "--- Building Rust binary for Raspberry Pi in Docker ---"

# Run the Docker container to compile the code.
docker run --rm -it \
  -u "$(id -u):$(id -g)" \
  -v "$(pwd)":/home/builder/project \
  -v rust-photobooth-cache:/home/builder/.cargo/registry \
  ${DOCKER_IMAGE_NAME} \
  cargo build --release --target aarch64-unknown-linux-gnu

echo ""
echo "--- Build complete. Deploying to Raspberry Pi at ${PI_HOST} ---"

LOCAL_BINARY_PATH="./target/aarch64-unknown-linux-gnu/release/${BINARY_NAME}"

echo "--- Starting cleanup of previous deployment ---"
ssh -i "${SSH_KEY_PATH}" "${PI_USER}@${PI_HOST}" << 'EOF'
    #cleanup tasks
    pkill -f rust_booth || true
    pkill -f kiosk_startup || true
    pkill -f chromium || true
    pkill -f wvkbd || true
    sleep 3

    # Stop PipeWire services that are using the camera
    systemctl --user stop pipewire-pulse || true
    systemctl --user stop wireplumber || true  
    systemctl --user stop pipewire || true
    sleep 2

    # Kill any camera processes
    pkill -f libcamera || true
    pkill -f rpicam || true
    sleep 1
    
    # Clean up temp files
    rm -f /tmp/kiosk_mode
    rm -rf /tmp/chromium-kiosk
    
    # Archive old logs

    for log_file in kiosk_*.log; do
        if [[ -f "$log_file" ]]; then
            if [[ "$log_file" =~ (kiosk)_([0-9]{8})_[0-9]{6}\.log ]]; then
                log_date="${BASH_REMATCH[2]}"
                daily_archive="logs/${BASH_REMATCH[1]}_${log_date}.tar.gz"
                
                mkdir -p logs
                echo "Archiving $log_file to $daily_archive"
                
                if [[ -f "$daily_archive" ]]; then
                    tar -xzf "$daily_archive" -C /tmp/ 2>/dev/null || true
                    tar -czf "$daily_archive" -C /tmp/ ${BASH_REMATCH[1]}_${log_date}_*.log "$log_file" 2>/dev/null || tar -czf "$daily_archive" "$log_file"
                    rm -f /tmp/${BASH_REMATCH[1]}_${log_date}_*.log 2>/dev/null || true
                else
                    tar -czf "$daily_archive" "$log_file"
                fi
                
                rm -f "$log_file"
            fi
        fi
    done

    rm -rf ${REMOTE_DEST_PATH}
EOF

echo "--- Deploying files to pi ---"
# Deploy the binary
scp -i "${SSH_KEY_PATH}" "${LOCAL_BINARY_PATH}" "${PI_USER}@${PI_HOST}:${REMOTE_DEST_PATH}"

# Deploy all the scripts from .ci/scripts
scp -i "${SSH_KEY_PATH}" .ci/scripts/kiosk_startup.sh "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"
scp -i "${SSH_KEY_PATH}" .ci/scripts/toggle_keyboard.sh "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"
scp -i "${SSH_KEY_PATH}" .ci/scripts/show_keyboard.sh "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"
scp -i "${SSH_KEY_PATH}" .ci/scripts/hide_keyboard.sh "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"
scp -i "${SSH_KEY_PATH}" .ci/desktop-settings/photobooth-kiosk.desktop "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"

# Deploy keyring disable files
scp -i "${SSH_KEY_PATH}" .ci/desktop-settings/gnome-keyring-*.desktop "${PI_USER}@${PI_HOST}:/home/${PI_USER}/"

echo "--- Setting up kiosk mode on Raspberry Pi ---"
ssh -i "${SSH_KEY_PATH}" "${PI_USER}@${PI_HOST}" << 'EOF'
    # Make scripts executable
    chmod +x /home/prospero/rust_booth
    chmod +x /home/prospero/kiosk_startup.sh
    chmod +x /home/prospero/toggle_keyboard.sh
    chmod +x /home/prospero/show_keyboard.sh
    chmod +x /home/prospero/hide_keyboard.sh
    
    # Create autostart directory if it doesn't exist
    mkdir -p /home/prospero/.config/autostart
    
    # Move desktop files to autostart
    mv /home/prospero/photobooth-kiosk.desktop /home/prospero/.config/autostart/
    mv /home/prospero/gnome-keyring-*.desktop /home/prospero/.config/autostart/
    
    # Install required packages including wvkbd
    sudo apt-get update
    sudo apt-get install -y chromium-browser unclutter curl build-essential git
    
    # Install wvkbd if not already installed
    if ! command -v wvkbd-mobintl >/dev/null 2>&1; then
        echo "Installing wvkbd..."
        cd /tmp
        git clone https://github.com/jjsullivan5196/wvkbd.git
        cd wvkbd
        make
        sudo make install
        cd /home/prospero
    fi
    
    # Create chromium temp directory
    mkdir -p /tmp/chromium-kiosk
    
    # Disable screen blanking in lightdm if it exists
    if [ -f /etc/lightdm/lightdm.conf ]; then
        sudo sed -i 's/#xserver-command=X/xserver-command=X -s 0 -dpms/' /etc/lightdm/lightdm.conf
    fi
    
    echo "Kiosk setup complete!"
    echo "The photo booth will start automatically on next boot."
    echo "To start now, run: /home/prospero/kiosk_startup.sh"
EOF

echo "--- Deployment complete. Restarting application on Raspberry Pi ---"

ssh -i "${SSH_KEY_PATH}" "${PI_USER}@${PI_HOST}" << 'EOF'
    export DISPLAY=:0
    nohup /home/prospero/kiosk_startup.sh >> kiosk_startup.log 2>&1 &
    echo "Kiosk mode started!"
    echo "Check logs with: tail -f /home/prospero/kiosk_*.log"
EOF

echo "--- Done. Application should be running on the Raspberry Pi. ---"
