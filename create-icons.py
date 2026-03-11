#!/usr/bin/env python3
"""
Simple icon generator for Tauri app.
Creates basic placeholder icons. Replace with proper icons in production.
"""

from PIL import Image, ImageDraw, ImageFont
import os

def create_icon(size, output_path):
    """Create a simple colored icon with VPN text"""
    # Create blue background
    img = Image.new('RGBA', (size, size), color='#3b82f6')
    draw = ImageDraw.Draw(img)

    # Try to use a default font
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", size // 3)
    except:
        try:
            font = ImageFont.truetype("arial.ttf", size // 3)
        except:
            font = ImageFont.load_default()

    # Draw text
    text = "V"
    bbox = draw.textbbox((0, 0), text, font=font)
    text_width = bbox[2] - bbox[0]
    text_height = bbox[3] - bbox[1]

    position = ((size - text_width) // 2, (size - text_height) // 2 - bbox[1])
    draw.text(position, text, fill='white', font=font)

    # Save
    img.save(output_path)
    print(f"Created {output_path}")

def create_ico(sizes, output_path):
    """Create Windows ICO file with multiple sizes"""
    images = []
    for size in sizes:
        img = Image.new('RGBA', (size, size), color='#3b82f6')
        draw = ImageDraw.Draw(img)

        try:
            font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", size // 3)
        except:
            font = ImageFont.load_default()

        text = "V"
        bbox = draw.textbbox((0, 0), text, font=font)
        text_width = bbox[2] - bbox[0]
        text_height = bbox[3] - bbox[1]

        position = ((size - text_width) // 2, (size - text_height) // 2 - bbox[1])
        draw.text(position, text, fill='white', font=font)

        images.append(img)

    # Save as ICO
    images[0].save(output_path, format='ICO', sizes=[(s, s) for s in sizes])
    print(f"Created {output_path}")

def main():
    icons_dir = os.path.join(os.path.dirname(__file__), 'src-tauri', 'icons')
    os.makedirs(icons_dir, exist_ok=True)

    # Change to icons directory
    os.chdir(icons_dir)

    # Create PNG icons
    create_icon(32, '32x32.png')
    create_icon(128, '128x128.png')
    create_icon(256, '128x128@2x.png')

    # Create ICO for Windows
    create_ico([16, 32, 48, 64, 128, 256], 'icon.ico')

    # Create a basic ICNS placeholder (just a large PNG)
    # For proper ICNS, use macOS iconutil
    create_icon(512, 'icon.icns')

    print("\nIcons created successfully!")
    print("Note: icon.icns is just a placeholder PNG. For proper macOS support,")
    print("use iconutil on macOS to create a proper ICNS file.")

if __name__ == '__main__':
    try:
        main()
    except ImportError:
        print("Error: PIL (Pillow) not found.")
        print("Install it with: pip install Pillow")
        print("\nAlternatively, you can use the Tauri CLI icon generator:")
        print("  npm run tauri icon path/to/icon.png")
