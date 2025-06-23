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
# We use the -u flag as you found it works for permissions.
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
ssh -i "${SSH_KEY_PATH}" "${PI_USER}@${PI_HOST}" << EOF
    #cleanup tasks
    pkill -f ${REMOTE_DEST_PATH}
    sleep 2

    # Stop PipeWire services that are using the camera
    systemctl --user stop pipewire-pulse || true
    systemctl --user stop wireplumber || true  
    systemctl --user stop pipewire || true
    sleep 2

    # Kill any camera processes
    pkill -f libcamera || true
    pkill -f rpicam || true
    sleep 1
    
    for log_file in rust_booth_*.log; do
      if [[ -f "$log_file" ]]; then
          # Extract date from filename (format: rust_booth_YYYYMMDD_HHMMSS.log)
          if [[ "$log_file" =~ rust_booth_([0-9]{8})_[0-9]{6}\.log ]]; then
              log_date="${BASH_REMATCH[1]}"
              daily_archive="${LOG_ARCHIVE_DIR}/rust_booth_${log_date}.tar.gz"
              
              echo "Archiving $log_file to $daily_archive"
              
              # If daily archive already exists, extract it first to add the new log
              if [[ -f "$daily_archive" ]]; then
                  tar -xzf "$daily_archive" -C /tmp/
                  tar -czf "$daily_archive" -C /tmp/ rust_booth_${log_date}_*.log "$log_file"
                  rm -f /tmp/rust_booth_${log_date}_*.log
              else
                  tar -czf "$daily_archive" "$log_file"
              fi
              
              # Remove the original log file after archiving
              rm -f "$log_file"
          fi
      fi
    done

    rm -rf ${REMOTE_DEST_PATH}
EOF

echo "--- Deploying files to pi ---"
scp -r -i "${SSH_KEY_PATH}" "${LOCAL_BINARY_PATH}" "${PI_USER}@${PI_HOST}:${REMOTE_DEST_PATH}"
echo "--- Deployment complete. Restarting application on Raspberry Pi ---"

ssh -i "${SSH_KEY_PATH}" "${PI_USER}@${PI_HOST}" << EOF
    chmod +x ${REMOTE_DEST_PATH}
    export DISPLAY=:0
    nohup ${REMOTE_DEST_PATH} >> ${LOG_FILE} 2>&1 &
    echo "Application started with PID: \$!"
    echo "Logs are being written to: ${LOG_FILE}"
EOF

echo "--- Done. Application should be running on the Raspberry Pi. ---"