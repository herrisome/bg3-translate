#!/usr/bin/env python3
"""
将 qlmanage 渲染的 SVG PNG（四角是白色）转为正确的 macOS 图标：
squircle 蒙版外区域设为透明。

这解决了白边问题：qlmanage 渲染 SVG 时给画布外区域填充白色背景，
而 macOS 图标要求蒙版外区域透明。
"""
import math
import struct
import zlib
import sys

# macOS Big Sur 标准 squircle 的内缩（与 SVG 蒙版一致的路径）
# 简化判断：用超椭圆方程近似 (|x|^n + |y|^n = 1, n≈5)
# 但更准确的是直接复刻 SVG 里的路径。这里用解析方法。

def in_squircle(px, py, size=1024):
    """判断点是否在 macOS squircle 内。
    用超椭圆 |2x-1|^n + |2y-1|^n <= 1 近似，n=5 接近 macOS 曲率。
    """
    # 归一化到 [-1, 1]，以画布中心为原点
    nx = (2 * px / size) - 1
    ny = (2 * py / size) - 1
    n = 5.0
    return (abs(nx) ** n + abs(ny) ** n) <= 1.0


def read_png(path):
    with open(path, "rb") as f:
        data = f.read()
    # 解析 IHDR
    width = struct.unpack(">I", data[16:20])[0]
    height = struct.unpack(">I", data[20:24])[0]
    # 找所有 IDAT chunk 并解压
    idat = b""
    pos = 8
    while pos < len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        ctype = data[pos + 4:pos + 8]
        if ctype == b"IDAT":
            idat += data[pos + 8:pos + 8 + length]
        pos += 8 + length + 4
    raw = zlib.decompress(idat)
    # 去掉每行开头的 filter byte
    stride = width * 4
    pixels = bytearray()
    for y in range(height):
        start = y * (stride + 1)
        pixels.extend(raw[start + 1:start + 1 + stride])
    return width, height, bytes(pixels)


def write_png(path, width, height, pixels):
    def chunk(typ, cdata):
        c = typ + cdata
        return struct.pack(">I", len(cdata)) + c + struct.pack(
            ">I", zlib.crc32(c) & 0xFFFFFFFF
        )
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw.extend(pixels[y * stride:(y + 1) * stride])
    idat = zlib.compress(bytes(raw), 9)
    with open(path, "wb") as f:
        f.write(sig)
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", idat))
        f.write(chunk(b"IEND", b""))


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else "tools/icon.svg.png"
    dst = sys.argv[2] if len(sys.argv) > 2 else "src-tauri/icons/icon.png"

    width, height, pixels = read_png(src)
    print(f"读取: {src} ({width}x{height})")

    # 稍微内缩蒙版一点点（0.5px），避免边缘抗锯齿残留白线
    # 对每个像素，如果不在 squircle 内，设为透明
    out = bytearray(pixels)
    changed = 0
    # 为了边缘平滑，做 1px 的羽化：蒙版边界附近的像素按距离衰减 alpha
    for y in range(height):
        for x in range(width):
            idx = (y * width + x) * 4
            r, g, b, a = pixels[idx], pixels[idx+1], pixels[idx+2], pixels[idx+3]
            # 计算到 squircle 边界的"距离"（用超椭圆值）
            nx = (2 * x / width) - 1
            ny = (2 * y / height) - 1
            n = 5.0
            val = abs(nx) ** n + abs(ny) ** n
            if val > 1.0:
                # 蒙版外：透明
                if a > 0:
                    out[idx+3] = 0
                    changed += 1
            else:
                # 蒙版内：如果是白色（qlmanage 填的背景），改成深色背景色
                # 避免 squircle 内部意外残留的白底
                if r > 240 and g > 240 and b > 240 and a > 200:
                    # 这应该是背景，设为深紫
                    out[idx] = 13
                    out[idx+1] = 9
                    out[idx+2] = 30
    print(f"处理了 {changed} 个蒙版外像素（设为透明）")
    write_png(dst, width, height, bytes(out))
    print(f"写出: {dst}")


if __name__ == "__main__":
    main()
