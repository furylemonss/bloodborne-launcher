from pathlib import Path
from PIL import Image

root = Path(r"D:\İndirilenler\Bloodborne\Installer\src-tauri\icons")
src = Image.open(r"D:\İndirilenler\Bloodborne\Installer\logo.png").convert("RGBA")
for name, size in [
    ("32x32.png", 32),
    ("64x64.png", 64),
    ("128x128.png", 128),
    ("128x128@2x.png", 256),
    ("icon.png", 512),
]:
    src.resize((size, size), Image.Resampling.LANCZOS).save(root / name, format="PNG", optimize=True)
src.save(
    root / "icon.ico",
    format="ICO",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
print("ok", (root / "icon.ico").stat().st_size)
