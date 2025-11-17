from PIL import Image 
import struct 
 
# Load the image 
try: 
    img = Image.open("background.png") 
    # Crop to 240x320 (centered) 
    left = (img.width - 240) // 2  # Center horizontally 
    top = (img.height - 320) // 2  # Center vertically 
    right = left + 240 
    bottom = top + 320 
    img = img.crop((left, top, right, bottom)) 
    img = img.convert("RGB")  # Convert to RGB 
    print("Image dimensions after cropping:", img.width, "x", img.height) 
except Exception as e: 
    print("Error loading or processing image:", e) 
    exit(1) 
 
# Convert to RGB565 
with open("background.raw", "wb") as f: 
    for y in range(img.height): 
        for x in range(img.width): 
            r, g, b = img.getpixel((x, y)) 
            print(f"Processing pixel at ({x}, {y})") 
            # Scale RGB values to 5-6-5 bits 
            r = (r * 31 // 255) & 0x1F 
            g = (g * 63 // 255) & 0x3F 
            b = (b * 31 // 255) & 0x1F 
            # Pack into RGB565 (16-bit) 
            rgb565 = (r << 11) | (g << 5) | b 
            # Write as little-endian 
            f.write(struct.pack("<H", rgb565)) 
