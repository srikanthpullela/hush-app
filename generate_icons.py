#!/usr/bin/env python3
"""Generate all Hush app icons — unified bell design."""

from PIL import Image, ImageDraw, ImageFont
import math
import os

ICONS_DIR = os.path.join(os.path.dirname(__file__), "src-tauri", "icons")

# ── Helpers ──────────────────────────────────────────────

def bezier(t, p0, p1, p2, p3):
    """Cubic bezier point at parameter t."""
    u = 1 - t
    return (
        u**3 * p0[0] + 3*u**2*t * p1[0] + 3*u*t**2 * p2[0] + t**3 * p3[0],
        u**3 * p0[1] + 3*u**2*t * p1[1] + 3*u*t**2 * p2[1] + t**3 * p3[1],
    )


def bell_outline(cx, cy, size, steps=80):
    """Return polygon points for a classic notification-bell silhouette."""
    s = size
    pts = []
    
    # Key reference points (relative to centre)
    # Top of dome
    top_y = cy - s * 0.38
    # Where dome meets body
    shoulder_y = cy - s * 0.08
    # Bell mouth (wide flare)
    mouth_y = cy + s * 0.20
    # Half-widths
    dome_hw = s * 0.22   # dome is narrow
    shoulder_hw = s * 0.24
    mouth_hw = s * 0.38  # mouth flares wide
    
    # LEFT side curve: top of dome → shoulder → mouth (going down)
    # Top dome (semicircle-ish via bezier)
    for i in range(steps):
        t = i / steps
        p = bezier(t,
            (cx, top_y),                              # top center
            (cx - dome_hw * 1.4, top_y),              # control: pull left
            (cx - dome_hw * 1.1, shoulder_y - s*0.1), # control
            (cx - shoulder_hw, shoulder_y),            # shoulder
        )
        pts.append(p)
    
    # LEFT shoulder → mouth flare (concave then convex curve, like a real bell)
    for i in range(steps):
        t = i / steps
        p = bezier(t,
            (cx - shoulder_hw, shoulder_y),
            (cx - shoulder_hw * 0.95, shoulder_y + s * 0.12),
            (cx - mouth_hw * 0.7, mouth_y - s * 0.10),
            (cx - mouth_hw, mouth_y),
        )
        pts.append(p)
    
    # Bottom: left mouth → right mouth
    pts.append((cx - mouth_hw, mouth_y))
    pts.append((cx + mouth_hw, mouth_y))
    
    # RIGHT mouth → shoulder (going up, mirror of left)
    for i in range(steps):
        t = i / steps
        p = bezier(t,
            (cx + mouth_hw, mouth_y),
            (cx + mouth_hw * 0.7, mouth_y - s * 0.10),
            (cx + shoulder_hw * 0.95, shoulder_y + s * 0.12),
            (cx + shoulder_hw, shoulder_y),
        )
        pts.append(p)
    
    # RIGHT shoulder → top (going up)
    for i in range(steps + 1):
        t = i / steps
        p = bezier(t,
            (cx + shoulder_hw, shoulder_y),
            (cx + dome_hw * 1.1, shoulder_y - s*0.1),
            (cx + dome_hw * 1.4, top_y),
            (cx, top_y),
        )
        pts.append(p)
    
    return pts


def draw_bell(draw, cx, cy, size, fill):
    """Draw a proper bell shape centred at (cx, cy) fitting in `size`."""
    s = size
    
    # Main bell body
    body = bell_outline(cx, cy, s)
    draw.polygon(body, fill=fill)
    
    # Brim bar at the mouth
    mouth_y = cy + s * 0.20
    brim_hw = s * 0.42
    brim_h = s * 0.055
    draw.rounded_rectangle(
        [cx - brim_hw, mouth_y - brim_h/2, cx + brim_hw, mouth_y + brim_h/2],
        radius=brim_h/2, fill=fill
    )
    
    # Clapper (small circle below)
    clap_r = s * 0.07
    clap_cy = cy + s * 0.33
    draw.ellipse(
        [cx - clap_r, clap_cy - clap_r, cx + clap_r, clap_cy + clap_r],
        fill=fill
    )
    
    # Knob on top
    nub_r = s * 0.04
    top_y = cy - s * 0.38
    nub_cy = top_y - nub_r * 0.4
    draw.ellipse(
        [cx - nub_r, nub_cy - nub_r, cx + nub_r, nub_cy + nub_r],
        fill=fill
    )


def make_gradient_bg(size, r1, r2):
    """Create a rounded-rect image with a gradient from r1 to r2 (top-left to bottom-right)."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    # Draw gradient
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * size)
            r = int(r1[0] + (r2[0] - r1[0]) * t)
            g = int(r1[1] + (r2[1] - r1[1]) * t)
            b = int(r1[2] + (r2[2] - r1[2]) * t)
            img.putpixel((x, y), (r, g, b, 255))
    
    # Apply rounded rect mask
    mask = Image.new("L", (size, size), 0)
    mask_draw = ImageDraw.Draw(mask)
    radius = size * 0.22
    mask_draw.rounded_rectangle([0, 0, size-1, size-1], radius=radius, fill=255)
    img.putalpha(mask)
    
    return img


def create_app_icon(size):
    """Modern bell icon with purple-blue gradient background."""
    # Gradient: purple (#7c3aed) to blue (#3b82f6)
    img = make_gradient_bg(size, (124, 58, 237), (59, 130, 246))
    draw = ImageDraw.Draw(img)
    
    # Draw white bell
    draw_bell(draw, size/2, size * 0.47, size * 0.7, fill=(255, 255, 255, 255))
    
    return img


def create_tray_icon(size, hushed=False, loading=False):
    """Tray icon — white on transparent for dark macOS menu bar. Large and bold."""
    # Use @2x canvas for crisp rendering (macOS expects 44px for 22pt @2x)
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    color = (255, 255, 255, 240)  # White for dark menu bar
    
    if loading:
        # Circular spinner — thick and bold
        cx, cy = size/2, size/2
        r = size * 0.38
        lw = max(3, int(size * 0.09))
        bbox = [cx - r, cy - r, cx + r, cy + r]
        # Background track (faint)
        draw.arc(bbox, start=0, end=360, fill=(255, 255, 255, 50), width=lw)
        # Active arc
        draw.arc(bbox, start=-90, end=150, fill=color, width=lw)
    else:
        # Bell icon — fill 90% of the canvas for maximum visibility
        draw_bell(draw, size/2, size * 0.46, size * 0.92, fill=color)
        
        if hushed:
            # Bold diagonal slash
            lw = max(3, int(size * 0.08))
            margin = size * 0.08
            # Dark knockout border for contrast
            draw.line(
                [(size - margin, margin), (margin, size - margin)],
                fill=(0, 0, 0, 255), width=lw + 5
            )
            # White slash
            draw.line(
                [(size - margin, margin), (margin, size - margin)],
                fill=color, width=lw
            )
    
    return img


# ── Generate all icons ───────────────────────────────────

os.makedirs(ICONS_DIR, exist_ok=True)

# App icon at various sizes
print("Generating app icons...")
icon_512 = create_app_icon(512)
icon_512.save(os.path.join(ICONS_DIR, "icon.png"))

for sz in [32, 128]:
    icon_512.resize((sz, sz), Image.LANCZOS).save(
        os.path.join(ICONS_DIR, f"{sz}x{sz}.png")
    )

# 128x128@2x = 256px
icon_512.resize((256, 256), Image.LANCZOS).save(
    os.path.join(ICONS_DIR, "128x128@2x.png")
)

# Windows square logos
for sz in [30, 44, 71, 89, 107, 142, 150, 284, 310]:
    icon_512.resize((sz, sz), Image.LANCZOS).save(
        os.path.join(ICONS_DIR, f"Square{sz}x{sz}Logo.png")
    )
icon_512.resize((50, 50), Image.LANCZOS).save(
    os.path.join(ICONS_DIR, "StoreLogo.png")
)

# macOS .icns from 512 PNG using sips
print("Generating icon.icns...")
os.system(
    f'sips -s format icns "{os.path.join(ICONS_DIR, "icon.png")}" '
    f'--out "{os.path.join(ICONS_DIR, "icon.icns")}" 2>/dev/null'
)

# Windows .ico
print("Generating icon.ico...")
ico_sizes = [16, 32, 48, 256]
ico_imgs = [icon_512.resize((s, s), Image.LANCZOS) for s in ico_sizes]
ico_imgs[0].save(
    os.path.join(ICONS_DIR, "icon.ico"),
    format="ICO",
    sizes=[(s, s) for s in ico_sizes],
    append_images=ico_imgs[1:]
)

# Tray icons (44px for @2x, macOS template style)
print("Generating tray icons...")
tray_size = 44

tray_normal = create_tray_icon(tray_size, hushed=False)
tray_normal.save(os.path.join(ICONS_DIR, "tray-normal.png"))

tray_hushed = create_tray_icon(tray_size, hushed=True)
tray_hushed.save(os.path.join(ICONS_DIR, "tray-hushed.png"))

tray_loading = create_tray_icon(tray_size, loading=True)
tray_loading.save(os.path.join(ICONS_DIR, "tray-loading.png"))

print("✅ All icons generated!")
