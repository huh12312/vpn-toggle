#!/bin/bash

# This script creates placeholder icon files for Tauri
# In production, replace these with proper icons

cd "$(dirname "$0")/src-tauri/icons"

# Create a simple SVG icon
cat > icon.svg << 'EOF'
<svg width="128" height="128" xmlns="http://www.w3.org/2000/svg">
  <rect width="128" height="128" fill="#3b82f6"/>
  <text x="64" y="64" font-size="48" fill="white" text-anchor="middle" dominant-baseline="middle">VPN</text>
</svg>
EOF

# Install imagemagick if needed for icon conversion
if ! command -v convert &> /dev/null; then
    echo "ImageMagick not found. Please install it to generate icons:"
    echo "  sudo apt-get install imagemagick (Ubuntu/Debian)"
    echo "  brew install imagemagick (macOS)"
    echo ""
    echo "Or use an online tool to create icons from icon.svg"
    exit 1
fi

# Generate PNG icons
convert icon.svg -resize 32x32 32x32.png
convert icon.svg -resize 128x128 128x128.png
convert icon.svg -resize 256x256 128x128@2x.png

# Generate ICO for Windows
convert 32x32.png 128x128.png -colors 256 icon.ico

# For macOS ICNS, we need iconutil (macOS only)
if command -v iconutil &> /dev/null; then
    mkdir -p icon.iconset
    convert icon.svg -resize 16x16 icon.iconset/icon_16x16.png
    convert icon.svg -resize 32x32 icon.iconset/icon_16x16@2x.png
    convert icon.svg -resize 32x32 icon.iconset/icon_32x32.png
    convert icon.svg -resize 64x64 icon.iconset/icon_32x32@2x.png
    convert icon.svg -resize 128x128 icon.iconset/icon_128x128.png
    convert icon.svg -resize 256x256 icon.iconset/icon_128x128@2x.png
    convert icon.svg -resize 256x256 icon.iconset/icon_256x256.png
    convert icon.svg -resize 512x512 icon.iconset/icon_256x256@2x.png
    convert icon.svg -resize 512x512 icon.iconset/icon_512x512.png
    convert icon.svg -resize 1024x1024 icon.iconset/icon_512x512@2x.png
    iconutil -c icns icon.iconset
    rm -rf icon.iconset
fi

echo "Icons generated successfully!"
