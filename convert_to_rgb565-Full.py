from PIL import Image 
import struct 
 
# Load the image 
try: 
    img = Image.open("background.png") 
 #   img = img.resize((240, 320))  # Resize to 86x86 if needed 
    img = img.convert("RGB")  # Convert to RGB 
except Exception as e: 
    print("Error loading or processing image:", e) 
    exit(1) 
 
# Convert to RGB565 
with open("background.raw", "wb") as f: 
    for y in range(img.height): 
        for x in range(img.width): 
            r, g, b = img.getpixel((x, y)) 
            # Scale RGB values to 5-6-5 bits 
            r = (r * 31 // 255) & 0x1F 
            g = (g * 63 // 255) & 0x3F 
            b = (b * 31 // 255) & 0x1F 
            # Pack into RGB565 (16-bit) 
            rgb565 = (r << 11) | (g << 5) | b 
            # Write as little-endian 
            f.write(struct.pack("<H", rgb565)) 
