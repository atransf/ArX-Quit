#!/bin/bash

set -e

APP_NAME="arxkill"
OLD_NAME="arx-quit"
INSTALL_DIR="/usr/local/bin"

installed=false
[ -f "$INSTALL_DIR/$APP_NAME" ] && installed=true

echo ""
echo "  ArX-Quit Setup"
echo "  ──────────────"
echo ""

if $installed; then
    echo "  Status: installed"
else
    echo "  Status: not installed"
fi

echo ""
echo "  1) Install"
echo "  2) Update"
echo "  3) Uninstall"
echo "  4) Cancel"
echo ""
printf "  Choose [1-4]: "
read -r choice

case "$choice" in
    1)
        if $installed; then
            echo ""
            echo "  $APP_NAME is already installed. Use option 2 to update."
            exit 0
        fi
        echo ""
        echo "Building $APP_NAME (release)..."
        cargo build --release
        echo "Installing to $INSTALL_DIR/$APP_NAME..."
        sudo cp "target/release/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
        sudo chmod +x "$INSTALL_DIR/$APP_NAME"
        # Clean up old binary if it exists
        if [ -f "$INSTALL_DIR/$OLD_NAME" ]; then
            echo "Removing old binary ($OLD_NAME)..."
            sudo rm "$INSTALL_DIR/$OLD_NAME"
        fi
        echo ""
        echo "Done. Run '$APP_NAME' from anywhere."
        ;;
    2)
        if ! $installed; then
            echo ""
            echo "  $APP_NAME is not installed yet. Use option 1 to install."
            exit 0
        fi
        echo ""
        echo "Building $APP_NAME (release)..."
        cargo build --release
        echo "Updating $INSTALL_DIR/$APP_NAME..."
        sudo cp "target/release/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
        sudo chmod +x "$INSTALL_DIR/$APP_NAME"
        echo ""
        echo "Done. $APP_NAME has been updated."
        ;;
    3)
        echo ""
        if $installed; then
            echo "Removing $INSTALL_DIR/$APP_NAME..."
            sudo rm "$INSTALL_DIR/$APP_NAME"
            echo "Done."
        else
            echo "$APP_NAME is not installed."
        fi
        # Also clean up old binary if it exists
        if [ -f "$INSTALL_DIR/$OLD_NAME" ]; then
            echo "Removing old binary ($OLD_NAME)..."
            sudo rm "$INSTALL_DIR/$OLD_NAME"
            echo "Done."
        fi
        ;;
    4)
        echo "Cancelled."
        ;;
    *)
        echo "Invalid choice."
        exit 1
        ;;
esac
