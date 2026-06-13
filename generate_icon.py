"""
生成最小的 ICO 图标文件（32x32，纯蓝色）
"""
import struct

def create_minimal_ico():
    # ICO 文件头
    ico_header = struct.pack('<HHH', 0, 1, 1)  # Reserved, Type (1=ICO), Count

    # 图像目录条目（32x32，32位色深）
    icon_dir = struct.pack('<BBBBHHII',
        32,      # Width
        32,      # Height
        0,       # Color count (0 for 32-bit)
        0,       # Reserved
        1,       # Color planes
        32,      # Bits per pixel
        0,       # Image size (will update)
        22       # Offset to image data
    )

    # 创建 32x32 的蓝色图像数据（BGRA 格式）
    width, height = 32, 32

    # BMP 信息头
    bmp_info = struct.pack('<IIIHHIIIIII',
        40,              # Header size
        width,           # Width
        height * 2,      # Height (doubled for ICO)
        1,               # Planes
        32,              # Bits per pixel
        0,               # Compression (0 = none)
        width * height * 4,  # Image size
        0, 0, 0, 0       # Other fields
    )

    # 像素数据（蓝色 + Alpha）
    pixel_data = b''
    for y in range(height):
        for x in range(width):
            # BGRA: 蓝色，完全不透明
            pixel_data += b'\xFF\x88\x44\xFF'  # 橙蓝渐变色

    # AND 掩码（全透明）
    and_mask = b'\x00' * (width * height // 8)

    # 组装图像数据
    image_data = bmp_info + pixel_data + and_mask

    # 更新图像大小
    icon_dir = struct.pack('<BBBBHHII',
        32, 32, 0, 0, 1, 32,
        len(image_data),
        22
    )

    # 组装完整的 ICO 文件
    ico_data = ico_header + icon_dir + image_data

    return ico_data

if __name__ == '__main__':
    ico_data = create_minimal_ico()
    with open('src-tauri/icons/icon.ico', 'wb') as f:
        f.write(ico_data)
    print('Icon generated: icon.ico')
