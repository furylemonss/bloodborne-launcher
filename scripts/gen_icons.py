import struct
import zlib
import os


def chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def write_png(path: str, w: int, h: int) -> None:
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            cx, cy = w / 2, h / 2
            dx, dy = (x - cx) / (w / 2), (y - cy) / (h / 2)
            dist = (dx * dx + dy * dy) ** 0.5
            if dist < 0.72:
                if dx * 0.9 + dy * 0.2 > 0.1 and dist < 0.52:
                    row.extend((255, 245, 235, 255))
                else:
                    t = max(0.0, min(1.0, (dx + 1) * 0.5))
                    r = int(255 * (1 - t * 0.25))
                    g = int(59 + (122 - 59) * t)
                    b = int(31 + (47 - 31) * t)
                    row.extend((r, g, b, 255))
            else:
                row.extend((12, 12, 16, 255))
        rows.append(bytes(row))
    raw = b"".join(rows)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def write_ico(path: str, png_path: str) -> None:
    with open(png_path, "rb") as f:
        png = f.read()
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png), 6 + 16)
    with open(path, "wb") as f:
        f.write(header + entry + png)


base = r"D:\İndirilenler\Bloodborne\Installer\src-tauri\icons"
write_png(os.path.join(base, "32x32.png"), 32, 32)
write_png(os.path.join(base, "128x128.png"), 128, 128)
write_png(os.path.join(base, "128x128@2x.png"), 256, 256)
write_png(os.path.join(base, "icon.png"), 512, 512)
write_ico(os.path.join(base, "icon.ico"), os.path.join(base, "icon.png"))
print("icons ok")
