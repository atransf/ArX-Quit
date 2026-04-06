#!/bin/bash

set -e

APP_NAME="arx-quit"
INSTALL_DIR="/usr/local/bin"

echo ""
echo "  ArX-Quit Setup"
echo "  ──────────────"
echo ""
echo "  1) Install"
echo "  2) Uninstall"
echo "  3) Cancel"
echo ""
printf "  Choose [1-3]: "
read -r choice

case "$choice" in
    1)
        echo ""
        echo "Building $APP_NAME (release)..."
        cargo build --release
        echo "Installing to $INSTALL_DIR/$APP_NAME..."
        sudo cp "target/release/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
        sudo chmod +x "$INSTALL_DIR/$APP_NAME"
        echo ""
        echo "Done. Run '$APP_NAME' from anywhere."
        ;;
    2)
        echo ""
        if [ -f "$INSTALL_DIR/$APP_NAME" ]; then
            echo "Removing $INSTALL_DIR/$APP_NAME..."
            sudo rm "$INSTALL_DIR/$APP_NAME"
            echo "Done."
        else
            echo "$APP_NAME is not installed."
        fi
        ;;
    3)
        echo "Cancelled."
        ;;
    *)
        echo "Invalid choice."
        exit 1
        ;;
esac
