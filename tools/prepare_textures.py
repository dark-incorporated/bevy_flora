#!/usr/bin/env python3
"""Prepare the CC0 texture assets bundled with bevy_flora.

Downloads, processes and writes into ``src/textures/``. Rerunnable: downloads
are cached in /tmp/bevy_flora_texture_cache/ (reused when present), outputs
are always regenerated. Needs python3, Pillow and numpy only.

Stages (``--only``): ``bark``, ``leaves``. Each stage is a function ``prepare_<stage>(root)``.
CREDITS.md and (for ``leaves``) src/textures/leaves.rs are rewritten on every run; CREDITS.md always
lists every stage, built from the static tables, so ``--only`` never drops rows.
The leaves stage caps each leaf's full outline at --max-outline-points (default 10) chosen by --selection
(default optimal: the best vertex subset of the 24-point reference outline; dp: Douglas-Peucker); the
defaults write the committed table, anything else needs --out-rs (and --sheet draws a comparison sheet).
This tool is repo-only and is not part of the published crate.
"""

import argparse
import io
import json
import math
import re
import ssl
import sys
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image


def _ssl_context():
    """Verifying context; python.org builds on macOS ship without a CA bundle."""
    try:
        import certifi

        return ssl.create_default_context(cafile=certifi.where())
    except ImportError:
        pass
    for cafile in ("/etc/ssl/cert.pem", "/etc/ssl/certs/ca-certificates.crt"):
        if Path(cafile).exists():
            return ssl.create_default_context(cafile=cafile)
    return ssl.create_default_context()


SSL_CONTEXT = _ssl_context()
CACHE = Path("/tmp/bevy_flora_texture_cache")
USER_AGENT = "bevy_flora-asset-prep (https://github.com/dark-incorporated/bevy_flora)"
BARK_BUDGET = 3_500_000
POLYHAVEN = "polyhaven"
AMBIENTCG = "ambientcg"
TEXTURECAN = "texturecan"
TEXTURECAN_PAGES = {"wood_0027": 221}  # asset id -> the number in https://www.texturecan.com/details/<n>/

# name (file stem), source, asset id, tile width x height in metres.
BARK_ROWS = [
    ("oak", POLYHAVEN, "jolcham_oak_bark_01", (1.0, 2.0)),
    ("pine", POLYHAVEN, "pine_bark", (2.0, 2.0)),
    ("willow", POLYHAVEN, "bark_willow", (1.0, 1.0)),
    # A procedural (Substance Designer) white birch: the tile is sized by eye, its lenticels and scars
    # at about their real size (a few centimetres) on a 0.6 m tile.
    ("birch", TEXTURECAN, "wood_0027", (0.6, 0.6)),
    ("palm", POLYHAVEN, "palm_bark", (1.86, 1.86)),
    ("cedar", POLYHAVEN, "japanese_cedar_bark", (1.0, 2.0)),
    ("zelkova", POLYHAVEN, "japanese_zelkova_bark", (1.8, 1.8)),
    ("maple", POLYHAVEN, "trident_maple_bark", (1.8, 1.8)),
    ("knotted_pine", POLYHAVEN, "knotted_pine_bark", (1.7, 1.7)),
    ("sakura", POLYHAVEN, "sakura_bark", (1.6, 1.6)),
    # 1:2 images whose Poly Haven `dimensions` are square: the round lichen blobs show the photo is
    # not stretched, so the tile is as tall as the image is.
    ("palm_tree", POLYHAVEN, "palm_tree_bark", (1.3, 2.6)),
    ("hackberry", POLYHAVEN, "chinese_hackberry_bark", (1.8, 3.6)),
    ("brown", POLYHAVEN, "bark_brown_02", (1.0, 1.0)),
    ("metasequoia", POLYHAVEN, "metasequoia_bark", (1.94, 1.94)),
]


# --------------------------------------------------------------------------
# Download helpers
# --------------------------------------------------------------------------


def fetch(url, cache_name, retries=3):
    """Return bytes of ``url``, cached under CACHE/cache_name."""
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / cache_name
    if path.exists() and path.stat().st_size > 0:
        return path.read_bytes()
    last = None
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=120, context=SSL_CONTEXT) as resp:
                data = resp.read()
            path.write_bytes(data)
            return data
        except (urllib.error.URLError, TimeoutError) as err:
            last = err
            time.sleep(1.5 * (attempt + 1))
    raise RuntimeError(f"download failed: {url}: {last}")


def fetch_json(url, cache_name):
    return json.loads(fetch(url, cache_name))


def to_rgb8(img):
    """Any Pillow mode (RGBA, P, L, 16-bit) to 8-bit RGB."""
    if img.mode in ("I;16", "I;16L", "I;16B", "I"):
        import numpy as np

        arr = np.asarray(img, dtype=np.float32)
        arr = arr / (65535.0 if arr.max() > 255 else 255.0)
        img = Image.fromarray((arr * 255.0 + 0.5).astype("uint8"), "L")
    elif img.mode in ("RGBA", "LA", "PA"):
        img = img.convert("RGBA")
        bg = Image.new("RGB", img.size, (0, 0, 0))
        bg.paste(img, mask=img.getchannel("A"))
        return bg
    return img.convert("RGB")


def resized(img, width):
    if img.width <= width:
        return img
    height = max(1, round(img.height * width / img.width))
    return img.resize((width, height), Image.LANCZOS)


def encode_jpeg(img, quality):
    buf = io.BytesIO()
    img.save(buf, "JPEG", quality=quality, optimize=True, progressive=False)
    return buf.getvalue()


# --------------------------------------------------------------------------
# Bark
# --------------------------------------------------------------------------


def load_bark_source(name, source, asset_id):
    """Return (colour PIL, normal PIL, author, page url, physical size in m or None)."""
    if source == POLYHAVEN:
        files = fetch_json(f"https://api.polyhaven.com/files/{asset_id}", f"ph_{asset_id}_files.json")
        info = fetch_json(f"https://api.polyhaven.com/info/{asset_id}", f"ph_{asset_id}_info.json")
        color_url = files["Diffuse"]["1k"]["jpg"]["url"]
        normal_url = files["nor_gl"]["1k"]["jpg"]["url"]
        color = Image.open(io.BytesIO(fetch(color_url, f"ph_{asset_id}_diff_1k.jpg")))
        normal = Image.open(io.BytesIO(fetch(normal_url, f"ph_{asset_id}_nor_gl_1k.jpg")))
        authors = ", ".join(info.get("authors", {}).keys()) or "Poly Haven"
        dims = info.get("dimensions")
        size_m = (dims[0] / 1000.0, dims[1] / 1000.0) if dims else None
        return to_rgb8(color), to_rgb8(normal), authors, f"https://polyhaven.com/a/{asset_id}", size_m
    if source == AMBIENTCG:
        zdata = fetch(f"https://ambientcg.com/get?file={asset_id}_1K-JPG.zip", f"acg_{asset_id}_1K-JPG.zip")
        with zipfile.ZipFile(io.BytesIO(zdata)) as zf:
            members = zf.namelist()
            color_m = next(m for m in members if m.endswith("_Color.jpg"))
            normal_m = next(m for m in members if m.endswith("_NormalGL.jpg"))
            color = Image.open(io.BytesIO(zf.read(color_m)))
            normal = Image.open(io.BytesIO(zf.read(normal_m)))
            color.load()
            normal.load()
        # The ambientCG API exposes no creator field; credit the library.
        return to_rgb8(color), to_rgb8(normal), "ambientCG", f"https://ambientcg.com/a/{asset_id}", None
    if source == TEXTURECAN:
        # The download link carries a hash, so it is read off the asset's page.
        page_url = f"https://www.texturecan.com/details/{TEXTURECAN_PAGES[asset_id]}/"
        page = fetch(page_url, f"tc_{asset_id}_page.html").decode("utf-8", "replace")
        link = re.search(rf'href="(/downloads/{asset_id}/{asset_id}_1k_[A-Za-z0-9]+\.zip)"', page)
        if link is None:
            raise RuntimeError(f"no 1K download link on {page_url}")
        zdata = fetch("https://www.texturecan.com" + link.group(1), f"tc_{asset_id}_1k.zip")
        with zipfile.ZipFile(io.BytesIO(zdata)) as zf:
            color = Image.open(io.BytesIO(zf.read(f"{asset_id}_color_1k.jpg")))
            normal = Image.open(io.BytesIO(zf.read(f"{asset_id}_normal_opengl_1k.png")))
            color.load()
            normal.load()
        return to_rgb8(color), to_rgb8(normal), "TextureCan", page_url, None
    raise ValueError(source)


def encode_bark_set(sources, width, q_color, q_normal, out_dir):
    """Write every pair; return {filename: size}."""
    sizes = {}
    dims = {}
    for name, (color, normal) in sources.items():
        c = resized(color, width)
        n = resized(normal, c.width)
        if n.size != c.size:
            n = n.resize(c.size, Image.LANCZOS)
        if c.size != n.size:
            raise RuntimeError(f"{name}: colour/normal size mismatch")
        for suffix, img, q in (("color", c, q_color), ("normal", n, q_normal)):
            data = encode_jpeg(img, q)
            fname = f"{name}_{suffix}.jpg"
            (out_dir / fname).write_bytes(data)
            sizes[fname] = len(data)
        dims[name] = c.size
    return sizes, dims


def prepare_bark(root):
    out_dir = root / "src" / "textures" / "bark"
    out_dir.mkdir(parents=True, exist_ok=True)
    for old in out_dir.glob("*.jpg"):
        old.unlink()

    sources = {}
    print("== bark: sources ==")
    for name, source, asset_id, (tw, th) in BARK_ROWS:
        color, normal, author, url, size_m = load_bark_source(name, source, asset_id)
        if color.size != normal.size:
            print(f"  note {name}: source colour {color.size} vs normal {normal.size}")
        sources[name] = (color, normal)
        if size_m:
            ratio = max(abs(size_m[0] / tw - 1), abs(size_m[1] / th - 1))
            flag = "  MISMATCH > 25%" if ratio > 0.25 else ""
            print(f"  {name:13s} {asset_id:26s} table {tw}x{th} m, polyhaven {size_m[0]:.2f}x{size_m[1]:.2f} m{flag}")
        else:
            print(f"  {name:13s} {asset_id:26s} table {tw}x{th} m, physical size not published")


    attempts = [
        ("quality 85/92, 512 px", 512, 85, 92),
        ("FALLBACK quality 80/88, 512 px", 512, 80, 88),
        ("FALLBACK quality 80/88, 384 px", 384, 80, 88),
    ]
    for label, width, qc, qn in attempts:
        sizes, dims = encode_bark_set(sources, width, qc, qn, out_dir)
        total = sum(sizes.values())
        print(f"== bark: encode {label}: total {total} bytes ==")
        if total <= BARK_BUDGET:
            break
    else:
        raise RuntimeError(f"bark set is {total} bytes, over the {BARK_BUDGET} budget even at 384 px")

    print("== bark: files ==")
    for fname in sorted(sizes):
        stem = fname.rsplit("_", 1)[0]
        print(f"  {fname:28s} {sizes[fname]:>9d} B  {dims[stem][0]}x{dims[stem][1]}")
    print(f"  bark total: {sum(sizes.values())} B (budget {BARK_BUDGET}); used: {label}")


def bark_credit_rows():
    rows = []
    for name, source, asset_id, (tw, th) in BARK_ROWS:
        if source == POLYHAVEN:
            info = fetch_json(f"https://api.polyhaven.com/info/{asset_id}", f"ph_{asset_id}_info.json")
            author = ", ".join(info.get("authors", {}).keys()) or "Poly Haven"
            url, library = f"https://polyhaven.com/a/{asset_id}", "Poly Haven"
        elif source == TEXTURECAN:
            author, url, library = "TextureCan", f"https://www.texturecan.com/details/{TEXTURECAN_PAGES[asset_id]}/", "TextureCan"
        else:
            author, url, library = "ambientCG", f"https://ambientcg.com/a/{asset_id}", "ambientCG"
        rows.append(
            {
                "files": f"bark/{name}_color.jpg, bark/{name}_normal.jpg",
                "asset": f"{asset_id} ({library})",
                "author": author,
                "url": url,
                "licence": "CC0 1.0",
                "tile": f"{tw} x {th}",
            }
        )
    return rows


# --------------------------------------------------------------------------
# Leaves
# --------------------------------------------------------------------------

LEAF_BUDGET = 1_600_000
ATLAS = 512  # atlas side in pixels, 2x2 cells
CELL = ATLAS // 2
LEAF_FIT = 240  # a leaf fits inside this many pixels of its 256 px cell, leaving room for the dilated border
DILATE_PX = 8
MIN_LEAF_AREA = 0.01  # fraction of the source image
REFERENCE_OUTLINE_POINTS = 24  # the area reference: the full outline simplified to this many points
DEFAULT_OUTLINE_POINTS = 10  # cap of the committed table's full outlines (24 and 16 points cost a forest of
# oaks 60-75% frame time, 12 about 13-18%; 10 points chosen by DEFAULT_SELECTION keep it within budget)
DEFAULT_SELECTION = "optimal"  # how the committed table's full outlines are chosen
COARSE_MIN_POINTS = 6  # fewer would reduce a lobed leaf to a quad
COARSE_POINTS = 8  # distant levels of detail use a coarser outline
COARSE_POINTS_MAX = 10  # allowed for a leaf that cannot keep its area with COARSE_POINTS
COARSE_AREA_TOL = 0.12
DEBUG_DIR = Path("/tmp/bevy_flora_leaf_debug")

# name (file stem), ambientCG set ids in order of preference (later ones only if an earlier set's
# leaves touch). The first id is what CREDITS.md lists: edit the list if a substitute is ever used.
LEAF_ROWS = [
    ("oak", ["LeafSet016", "LeafSet012"]),
    ("willow", ["LeafSet013"]),
    ("birch", ["LeafSet014"]),
    ("bonsai", ["LeafSet022"]),
    ("maple", ["LeafSet010", "LeafSet027"]),
    ("poplar", ["LeafSet024"]),
    ("japanese_maple", ["LeafSet021"]),
]


def load_leaf_set(set_id):
    """Return (colour RGB, opacity L, normal RGB) PIL images of an ambientCG leaf set."""
    data = fetch(f"https://ambientcg.com/get?file={set_id}_1K-PNG.zip", f"acg_{set_id}_1K-PNG.zip")
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        members = zf.namelist()

        def read(suffix, mode):
            member = next(m for m in members if m.endswith(suffix))
            img = Image.open(io.BytesIO(zf.read(member)))
            img.load()
            return img.convert(mode)

        return read("_Color.png", "RGB"), read("_Opacity.png", "L"), read("_NormalGL.png", "RGB")


def label_components(mask):
    """8-connected components of a boolean mask, as a list of arrays of (row, col) flat indices."""
    h, w = mask.shape
    padded = np.zeros((h + 2, w + 2), dtype=bool)
    padded[1:-1, 1:-1] = mask
    flat = padded.ravel()
    stride = w + 2
    offsets = [-stride - 1, -stride, -stride + 1, -1, 1, stride - 1, stride, stride + 1]
    seen = np.zeros(flat.size, dtype=bool)
    comps = []
    for start in np.flatnonzero(flat).tolist():
        if seen[start]:
            continue
        seen[start] = True
        stack = [start]
        members = []
        while stack:
            p = stack.pop()
            members.append(p)
            for o in offsets:
                q = p + o
                if flat[q] and not seen[q]:
                    seen[q] = True
                    stack.append(q)
        m = np.array(members)
        rows, cols = np.divmod(m, stride)
        comps.append((rows - 1, cols - 1))
    return comps


def dilate_fill(arr, valid, iters):
    """Extend the values of `valid` pixels `iters` pixels outward (mean of valid 8-neighbours)."""
    arr = arr.copy()
    valid = valid.copy()
    h, w = valid.shape
    for _ in range(iters):
        v = valid.astype(np.float32)
        pv = np.pad(v, 1)
        pa = np.pad(arr * v[..., None], ((1, 1), (1, 1), (0, 0)))
        cnt = np.zeros_like(v)
        acc = np.zeros_like(arr)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if dy == 0 and dx == 0:
                    continue
                cnt += pv[1 + dy : 1 + dy + h, 1 + dx : 1 + dx + w]
                acc += pa[1 + dy : 1 + dy + h, 1 + dx : 1 + dx + w]
        new = (~valid) & (cnt > 0)
        arr[new] = acc[new] / cnt[new][:, None]
        valid |= new
    return arr, valid


def grow_mask(mask, px):
    h, w = mask.shape
    out = mask.copy()
    for _ in range(px):
        p = np.pad(out, 1)
        grown = out.copy()
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                grown |= p[1 + dy : 1 + dy + h, 1 + dx : 1 + dx + w]
        out = grown
    return out


def petiole_len(seq):
    """Rows of a petiole at the start of a width profile: a run of near-constant narrow rows that jumps
    to the blade (a tapering tip widens smoothly instead). None if the end has no such stalk."""
    need = max(4, round(0.012 * len(seq)))
    for i in range(2, len(seq)):  # the first two rows are the antialiased end
        run = seq[2:i]
        if i >= 2 + need and seq[i] >= 2.5 * np.median(run):
            return i if run.max() <= 2.0 * run.min() else None
        if i > 2 and run.max() > 2.0 * run.min():
            return None
    return None


def extract_leaf(color, opacity, normal, comp):
    """Cut one leaf out of the sheet and orient it: principal axis vertical, petiole end at the bottom.

    Returns float arrays (rgb 0..255, alpha 0..1, normal rgb 0..255) cropped to the leaf, with the
    petiole trimmed to a short stub.
    """
    rows, cols = comp
    mask_full = np.zeros(opacity.shape, dtype=bool)
    mask_full[rows, cols] = True
    pad = 6
    y0, y1 = max(rows.min() - pad, 0), min(rows.max() + pad + 1, opacity.shape[0])
    x0, x1 = max(cols.min() - pad, 0), min(cols.max() + pad + 1, opacity.shape[1])
    leaf_mask = grow_mask(mask_full[y0:y1, x0:x1], 3)
    alpha = np.where(leaf_mask, opacity[y0:y1, x0:x1], 0).astype(np.uint8)
    rgb = color[y0:y1, x0:x1]
    nrm = normal[y0:y1, x0:x1]

    # Axis: PCA over the blade's pixels (y-up frame; the blade is the leaf minus thin stalks, found by a
    # morphological opening, so a long petiole cannot tilt it). A round leaf (maple, a round poplar) has
    # no stable principal axis: there the axis is the line from the largest thin remainder (the petiole,
    # or failing that the tip) to the blade's centre. Only the line matters; which end is the base is
    # decided below.
    small = mask_full[y0:y1, x0:x1]
    k = max(2, round(0.02 * math.sqrt(len(rows))))
    blade = grow_mask(~grow_mask(~small, k), k) & small
    if not blade.any():
        blade = small
    by, bx = np.nonzero(blade)
    pts = np.stack([bx.astype(np.float64), -by.astype(np.float64)], axis=1)
    pts -= pts.mean(axis=0)
    evals, evecs = np.linalg.eigh(pts.T @ pts / len(pts))
    axis = evecs[:, 1]
    angle = None
    if evals[1] < 1.9 * evals[0]:
        rest = small & ~grow_mask(blade, 2)
        stalk_px = 0
        if rest.any():
            sy, sx = max(label_components(rest), key=lambda p: len(p[0]))
            stalk_px = len(sy)
            axis = np.array([bx.mean() - sx.mean(), -(by.mean() - sy.mean())])
        angle = 90.0 - math.degrees(math.atan2(axis[1], axis[0]))
        # A thin remainder too small to be a petiole is a lobe or the tip, so the axis is only roughly
        # right. If neither end shows a petiole, take the nearest rotation (within 25 degrees, then
        # within 25 degrees of the opposite) that stands a petiole up at the bottom.
        mask_img = Image.fromarray(small.astype(np.uint8) * 255)

        def profile(a):
            m = np.asarray(mask_img.rotate(a, resample=Image.BILINEAR, expand=True)) > 127
            ys = np.flatnonzero(m.any(axis=1))
            return m[ys.min() : ys.max() + 1].sum(axis=1).astype(np.float64)

        w = profile(angle)
        if stalk_px < 0.01 * small.sum() and petiole_len(w) is None and petiole_len(w[::-1]) is None:
            for centre in (angle, angle + 180.0):
                hit = next(
                    (
                        centre + d
                        for d in (0, 2, -2, 4, -4, 6, -6, 8, -8, 10, -10, 12, -12, 14, -14, 16, -16, 18, -18, 20, -20, 22, -22, 24, -24)
                        if petiole_len(profile(centre + d)[::-1]) is not None
                    ),
                    None,
                )
                if hit is not None:
                    angle = hit
                    break
    if angle is None:
        angle = 90.0 - math.degrees(math.atan2(axis[1], axis[0]))  # rotation that makes the axis vertical

    def rot(img, fill):
        return img.rotate(angle, resample=Image.BICUBIC, expand=True, fillcolor=fill)

    rgb_r = np.asarray(rot(Image.fromarray(rgb), (0, 0, 0)), dtype=np.float32)
    # The blade mask travels as a 4th channel (not an RGBA image: Pillow premultiplies those).
    blade_r = np.asarray(rot(Image.fromarray(blade.astype(np.uint8) * 255), 0), dtype=np.float32)
    rgb_r = np.dstack([rgb_r, blade_r])
    alpha_r = np.asarray(rot(Image.fromarray(alpha), 0), dtype=np.float32) / 255.0
    nrm_r = np.asarray(rot(Image.fromarray(nrm), (128, 128, 255)), dtype=np.float32)
    # The tangent-space xy components rotate with the image (GL convention: x right, y up).
    theta = math.radians(angle)
    c, s = math.cos(theta), math.sin(theta)
    vx = nrm_r[..., 0] / 255.0 * 2.0 - 1.0
    vy = nrm_r[..., 1] / 255.0 * 2.0 - 1.0
    nrm_r[..., 0] = np.clip((vx * c - vy * s) * 0.5 + 0.5, 0, 1) * 255.0
    nrm_r[..., 1] = np.clip((vx * s + vy * c) * 0.5 + 0.5, 0, 1) * 255.0

    def crop(rgb_, alpha_, nrm_):
        ys, xs = np.nonzero(alpha_ > 0.5)
        sl = (slice(ys.min(), ys.max() + 1), slice(xs.min(), xs.max() + 1))
        return rgb_[sl], alpha_[sl], nrm_[sl]

    rgb_r, alpha_r, nrm_r = crop(rgb_r, alpha_r, nrm_r)

    # Which end is the base? The petiole end starts with a run of near-constant, narrow rows that then
    # jumps to the blade, while a tapering tip widens smoothly. If both ends (or neither) show that
    # signature, take the longer run (else the end whose last 10% of the length is narrower).
    widths = (alpha_r > 0.5).sum(axis=1).astype(np.float64)
    top, bottom = petiole_len(widths), petiole_len(widths[::-1])
    if top is not None and bottom is not None:
        base_at_top = top > bottom
    elif top is not None or bottom is not None:
        base_at_top = top is not None
    else:
        n10 = max(1, len(widths) // 10)
        base_at_top = widths[:n10].mean() < widths[-n10:].mean()
    if base_at_top:
        rgb_r, alpha_r, nrm_r = rgb_r[::-1, ::-1], alpha_r[::-1, ::-1], nrm_r[::-1, ::-1].copy()
        nrm_r[..., 0] = 255.0 - nrm_r[..., 0]  # a half turn negates the xy components
        nrm_r[..., 1] = 255.0 - nrm_r[..., 1]
        widths = widths[::-1]

    # Cut away everything that is not blade except a short stub of petiole (or twig), then trim the
    # tapering base: keep a 2% stub below the first row (from the base) as wide as 30% of the widest.
    stub = max(3, round(0.03 * math.sqrt(len(rows))))
    alpha_r = np.where(grow_mask(rgb_r[..., 3] > 127, stub), alpha_r, 0)
    rgb_r, alpha_r, nrm_r = crop(rgb_r, alpha_r, nrm_r)
    widths = (alpha_r > 0.5).sum(axis=1).astype(np.float64)
    length = len(widths)
    wide = np.flatnonzero(widths >= 0.3 * widths.max())
    cut = min(length, int(wide.max()) + 1 + max(1, round(0.02 * length)))
    rgb_r, alpha_r, nrm_r = rgb_r[:cut], alpha_r[:cut], nrm_r[:cut]
    rgb_r, alpha_r, nrm_r = crop(rgb_r, alpha_r, nrm_r)
    rgb_r = rgb_r[..., :3]

    # Colour and normal behind the soft alpha edge come from the leaf itself, so scaling cannot drag in
    # the sheet's background.
    valid = alpha_r > 0.9
    rgb_r, _ = dilate_fill(np.ascontiguousarray(rgb_r), valid, 12)
    nrm_r, _ = dilate_fill(np.ascontiguousarray(nrm_r), valid, 12)
    return rgb_r, np.ascontiguousarray(alpha_r), nrm_r


def build_atlas(name, set_id):
    """Return the atlas arrays of one set, or None if its leaves touch (fewer than 4 separate ones)."""
    color_img, opacity_img, normal_img = load_leaf_set(set_id)
    color = np.asarray(color_img)
    opacity = np.asarray(opacity_img)
    normal = np.asarray(normal_img)
    comps = label_components(opacity > 127)
    big = sorted((c for c in comps if len(c[0]) > MIN_LEAF_AREA * opacity.size), key=lambda c: -len(c[0]))
    areas = [len(c[0]) for c in big]
    print(f"  {name:15s} {set_id}: {len(comps)} components, {len(big)} above 1% of the image, areas {areas}")
    if len(big) < 4:
        return None
    kept = big[:4]

    atlas_rgb = np.zeros((ATLAS, ATLAS, 3), dtype=np.float32)
    atlas_nrm = np.zeros((ATLAS, ATLAS, 3), dtype=np.float32)
    atlas_alpha = np.zeros((ATLAS, ATLAS), dtype=np.float32)
    for k, comp in enumerate(kept):
        rgb, alpha, nrm = extract_leaf(color, opacity, normal, comp)
        h, w = alpha.shape
        scale = min(LEAF_FIT / w, LEAF_FIT / h)
        nw, nh = max(1, round(w * scale)), max(1, round(h * scale))
        rgb_s = np.asarray(Image.fromarray(rgb.round().astype(np.uint8)).resize((nw, nh), Image.LANCZOS), dtype=np.float32)
        nrm_s = np.asarray(Image.fromarray(nrm.round().astype(np.uint8)).resize((nw, nh), Image.LANCZOS), dtype=np.float32)
        alpha_s = np.asarray(
            Image.fromarray((alpha * 255).round().astype(np.uint8)).resize((nw, nh), Image.LANCZOS), dtype=np.float32
        ) / 255.0
        ox = (k % 2) * CELL + (CELL - nw) // 2
        oy = (k // 2) * CELL + (CELL - nh) // 2
        atlas_rgb[oy : oy + nh, ox : ox + nw] = rgb_s
        atlas_nrm[oy : oy + nh, ox : ox + nw] = nrm_s
        atlas_alpha[oy : oy + nh, ox : ox + nw] = alpha_s
        print(f"    leaf {k}: area {areas[k]} px, oriented crop {w}x{h}, in atlas {nw}x{nh}")

    # Extend colour and normal DILATE_PX past the alpha edge; the rest is the mean colour / flat normal.
    valid = atlas_alpha > 0.95
    mean_rgb = atlas_rgb[valid].mean(axis=0)
    atlas_rgb, filled = dilate_fill(atlas_rgb, valid, DILATE_PX + 1)
    atlas_nrm, _ = dilate_fill(atlas_nrm, valid, DILATE_PX + 1)
    atlas_rgb[~filled] = mean_rgb
    atlas_nrm[~filled] = (128, 128, 255)
    return atlas_rgb, atlas_nrm, atlas_alpha


# -- outlines ---------------------------------------------------------------


def marching_squares(a, level=0.5):
    """Closed iso-contours of `a` at `level` as arrays of (x, y) sample coordinates (pixel centres at
    integer coordinates). The array's border must be below `level`."""
    inside = a >= level
    tl, tr, br, bl = inside[:-1, :-1], inside[:-1, 1:], inside[1:, 1:], inside[1:, :-1]
    case = tl * 8 + tr * 4 + br * 2 + bl * 1
    center = (a[:-1, :-1] + a[:-1, 1:] + a[1:, 1:] + a[1:, :-1]) * 0.25 >= level
    adj = {}

    def link(e1, e2):
        adj.setdefault(e1, []).append(e2)
        adj.setdefault(e2, []).append(e1)

    rr, cc = np.nonzero((case > 0) & (case < 15))
    for r, c in zip(rr.tolist(), cc.tolist()):
        k = int(case[r, c])
        T, B, L, R = ("h", r, c), ("h", r + 1, c), ("v", r, c), ("v", r, c + 1)
        if k in (1, 14):
            link(L, B)
        elif k in (2, 13):
            link(B, R)
        elif k in (3, 12):
            link(L, R)
        elif k in (4, 11):
            link(T, R)
        elif k in (6, 9):
            link(T, B)
        elif k in (7, 8):
            link(T, L)
        elif k == 5:  # top-right and bottom-left inside
            if center[r, c]:
                link(T, L)
                link(B, R)
            else:
                link(T, R)
                link(L, B)
        elif k == 10:  # top-left and bottom-right inside
            if center[r, c]:
                link(T, R)
                link(L, B)
            else:
                link(T, L)
                link(B, R)

    def position(e):
        kind, r, c = e
        if kind == "h":
            t = (level - a[r, c]) / (a[r, c + 1] - a[r, c])
            return (c + t, float(r))
        t = (level - a[r, c]) / (a[r + 1, c] - a[r, c])
        return (float(c), r + t)

    visited = set()
    loops = []
    for start in adj:
        if start in visited:
            continue
        loop = [start]
        visited.add(start)
        prev, cur = None, start
        while True:
            n0, n1 = adj[cur][0], adj[cur][1]
            nxt = n0 if n0 != prev else n1
            if nxt == start:
                break
            loop.append(nxt)
            visited.add(nxt)
            prev, cur = cur, nxt
        loops.append(np.array([position(e) for e in loop]))
    return loops


def rdp(pts, eps):
    """Douglas-Peucker over an open polyline; returns the kept indices (ends included)."""
    keep = {0, len(pts) - 1}
    stack = [(0, len(pts) - 1)]
    while stack:
        i, j = stack.pop()
        if j <= i + 1:
            continue
        a, b = pts[i], pts[j]
        d = b - a
        seg = pts[i + 1 : j] - a
        norm = math.hypot(d[0], d[1])
        if norm < 1e-12:
            dist = np.hypot(seg[:, 0], seg[:, 1])
        else:
            dist = np.abs(seg[:, 0] * d[1] - seg[:, 1] * d[0]) / norm
        k = int(np.argmax(dist))
        if dist[k] > eps:
            m = i + 1 + k
            keep.add(m)
            stack.append((i, m))
            stack.append((m, j))
    return sorted(keep)


def simplify_closed(pts, eps):
    d = ((pts - pts[0]) ** 2).sum(axis=1)
    far = int(np.argmax(d))
    first = rdp(pts[: far + 1], eps)
    second = rdp(np.vstack([pts[far:], pts[:1]]), eps)
    idx = first[:-1] + [far + i for i in second[:-1]]
    return pts[idx]


def _orient(a, b, c):
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def _segments_intersect(p1, p2, p3, p4):
    d1, d2 = _orient(p3, p4, p1), _orient(p3, p4, p2)
    d3, d4 = _orient(p1, p2, p3), _orient(p1, p2, p4)
    if ((d1 > 0) != (d2 > 0)) and ((d3 > 0) != (d4 > 0)) and d1 != 0 and d2 != 0 and d3 != 0 and d4 != 0:
        return True

    def on(p, q, r):  # r on segment pq, given collinear
        return min(p[0], q[0]) <= r[0] <= max(p[0], q[0]) and min(p[1], q[1]) <= r[1] <= max(p[1], q[1])

    return (
        (d1 == 0 and on(p3, p4, p1))
        or (d2 == 0 and on(p3, p4, p2))
        or (d3 == 0 and on(p1, p2, p3))
        or (d4 == 0 and on(p1, p2, p4))
    )


def is_simple(poly):
    n = len(poly)
    if n < 3:
        return False
    for i in range(n):
        for j in range(i + 1, n):
            if j == i + 1 or (i == 0 and j == n - 1):
                continue  # adjacent edges share an endpoint
            if _segments_intersect(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]):
                return False
    # adjacent edges must not overlap back on themselves
    for i in range(n):
        a, b, c = poly[i - 1], poly[i], poly[(i + 1) % n]
        if _orient(a, b, c) == 0 and ((b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1])) < 0:
            return False
    return True


def polygon_area(poly):
    return 0.5 * sum(
        poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1] for i in range(len(poly))
    )


def triangulate(poly):
    """Ear clipping of a counter-clockwise simple polygon; returns index triples (all ccw)."""

    def inside(p, a, b, c):
        return _orient(a, b, p) >= 0 and _orient(b, c, p) >= 0 and _orient(c, a, p) >= 0

    idx = list(range(len(poly)))
    tris = []
    while len(idx) > 3:
        m = len(idx)
        for k in range(m):
            i0, i1, i2 = idx[k - 1], idx[k], idx[(k + 1) % m]
            a, b, c = poly[i0], poly[i1], poly[i2]
            area2 = _orient(a, b, c)
            if abs(area2) < 1e-12:
                del idx[k]  # collinear vertex: contributes no area
                break
            if area2 < 0:
                continue  # reflex
            if any(inside(poly[j], a, b, c) for j in idx if j not in (i0, i1, i2)):
                continue
            tris.append((i0, i1, i2))
            del idx[k]
            break
        else:
            raise RuntimeError("ear clipping found no ear")
    a, b, c = (poly[i] for i in idx)
    if _orient(a, b, c) > 1e-12:
        tris.append(tuple(idx))
    return tris


def visvalingam(pts, count):
    """Drop the vertex whose removal changes the polygon area least until `count` remain (indices)."""
    idx = list(range(len(pts)))
    while len(idx) > count:
        m = len(idx)
        costs = [abs(_orient(pts[idx[k - 1]], pts[idx[k]], pts[idx[(k + 1) % m]])) for k in range(m)]
        del idx[int(np.argmin(costs))]
    return idx


def coarse_outline(poly):
    """A subset of the (ccw, simple) full outline for distant levels of detail, with COARSE_MIN_POINTS to
    COARSE_POINTS points. Douglas-Peucker with the smallest epsilon that leaves at most COARSE_POINTS
    points (when that leaves fewer than COARSE_MIN_POINTS, Visvalingam, least area change first, at
    exactly that limit instead); if the polygon is not simple or its area is off by more than
    COARSE_AREA_TOL (a deeply lobed leaf), the same is tried with COARSE_POINTS_MAX points, and failing
    that Visvalingam at COARSE_POINTS, then COARSE_POINTS_MAX. Returns indices into `poly`, triangles,
    area ratio, and the method used."""
    pts = np.array(poly)
    index = {tuple(p): i for i, p in enumerate(poly)}
    full_area = polygon_area(poly)
    tried = []

    def candidates():
        for limit in (COARSE_POINTS, COARSE_POINTS_MAX):
            eps = 0.002
            while eps < 2.0:
                cp = [tuple(p) for p in simplify_closed(pts, eps)]
                if len(cp) <= limit:
                    if len(cp) >= COARSE_MIN_POINTS:
                        yield cp, f"douglas-peucker {limit}"
                    else:
                        yield [poly[i] for i in sorted(visvalingam(poly, min(limit, len(poly))))], f"visvalingam {limit}"
                    break
                eps *= 1.03
        for limit in (COARSE_POINTS, COARSE_POINTS_MAX):
            yield [poly[i] for i in sorted(visvalingam(poly, min(limit, len(poly))))], f"visvalingam {limit}"

    for cp, method in candidates():
        if len(cp) < 3 or not is_simple(cp):
            tried.append((method, "not simple"))
            continue
        ratio = polygon_area(cp) / full_area
        tried.append((method, round(ratio, 3)))
        if abs(ratio - 1.0) > COARSE_AREA_TOL:
            continue
        tris = triangulate(cp)
        area = polygon_area(cp)
        tri_area = sum(0.5 * _orient(cp[i], cp[j], cp[k]) for i, j, k in tris)
        if abs(tri_area - area) > 0.005 * area:
            raise RuntimeError(f"coarse triangulation area {tri_area} != polygon area {area}")
        if any(_orient(cp[i], cp[j], cp[k]) <= 0 for i, j, k in tris):
            raise RuntimeError("clockwise coarse triangle")
        return {"idx": [index[p] for p in cp], "triangles": [i for t in tris for i in t], "ratio": ratio, "method": method}
    raise RuntimeError(f"no coarse outline within the area tolerance (tried {tried})")


def simplify_to(full, limit):
    """Douglas-Peucker of the closed contour with the smallest epsilon (on a 1.1x ladder from 0.3 px) that
    leaves at most `limit` points and a simple polygon; None if there is none."""
    eps = 0.3
    while eps < 20.0:
        simp = simplify_closed(full, eps)
        if len(simp) <= limit and is_simple([tuple(p) for p in simp]):
            return simp
        eps *= 1.1
    return None


def outline_tolerance(max_points):
    """Largest allowed relative area deviation of a `max_points` outline from the reference outline."""
    if max_points >= REFERENCE_OUTLINE_POINTS:
        return 0.0
    if max_points >= 16:
        return 0.04
    if max_points >= 12:
        return 0.08
    if max_points >= 10:
        return 0.10
    return 0.15


# -- optimal vertex subsets -------------------------------------------------


def _grid(n=700):
    xs = (np.arange(n) + 0.5) / n * 2.0 - 1.0
    return np.meshgrid(xs, xs)


def _winding(poly, X, Y):
    """Winding number of the closed polygon around every grid point."""
    w = np.zeros(X.shape, dtype=np.int32)
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        if y1 == y2:
            continue
        side = (x2 - x1) * (Y - y1) - (y2 - y1) * (X - x1)  # > 0: point left of the edge
        up = (y1 <= Y) & (Y < y2)
        down = (y2 <= Y) & (Y < y1)
        w += (up & (side > 0)).astype(np.int32)
        w -= (down & (side < 0)).astype(np.int32)
    return w


def sym_diff_ratio(ref, q, n=700):
    """Area of the symmetric difference of two polygons (card space, x and y in -1..1) over the area of
    `ref`, by sampling an n x n grid."""
    X, Y = _grid(n)
    a = _winding(ref, X, Y) != 0
    b = _winding(q, X, Y) != 0
    return float((a ^ b).sum() / a.sum())


def _loop_area(loop):
    """Area covered by a closed loop counted without sign (sum of |winding number|): the area of a simple
    loop, or of both lobes of a crossing one."""
    x0, y0 = np.min(loop, axis=0)
    x1, y1 = np.max(loop, axis=0)
    if x1 - x0 < 1e-9 or y1 - y0 < 1e-9:
        return 0.0
    n = 240
    xs = x0 + (np.arange(n) + 0.5) / n * (x1 - x0)
    ys = y0 + (np.arange(n) + 0.5) / n * (y1 - y0)
    X, Y = np.meshgrid(xs, ys)
    return float(np.abs(_winding(loop, X, Y)).sum() * (x1 - x0) * (y1 - y0) / (n * n))


def _chain_cost(seq, i, j):
    """Area between the boundary chain seq[i..j] and its chord: |signed area| of the closed loop, or, if
    the chord crosses its own chain, the sum of the lobes' areas."""
    loop = [seq[k] for k in range(i, j + 1)]
    if len(loop) == 2:
        return 0.0
    chord_a, chord_b = loop[-1], loop[0]
    crossing = any(
        _segments_intersect(loop[k], loop[k + 1], chord_a, chord_b) for k in range(1, len(loop) - 2)
    )
    if not crossing:
        return abs(polygon_area(loop))
    return _loop_area(loop)


def optimal_subset(poly, count, tol, ref_area):
    """The `count`-vertex subset of the (ccw, simple) polygon that keeps the base (lowest) and tip (highest)
    vertices and minimises the area of the symmetric difference with `poly`, among the subsets that are
    simple polygons, triangulate, and keep their area within `tol` of `ref_area`. Subsets are ranked by
    the sum, over the boundary chains they cut off, of the area between each chain and its chord (exact for
    a simple subset), exhaustively. Returns indices into `poly`, or None."""
    from itertools import combinations

    m = len(poly)
    if count >= m:
        return list(range(m))
    base = min(range(m), key=lambda k: poly[k][1])
    tip = max(range(m), key=lambda k: poly[k][1])
    seq = [poly[(base + k) % m] for k in range(m)] + [poly[base]]
    t = (tip - base) % m
    cost = np.full((m + 1, m + 1), np.inf)
    for i in range(m + 1):
        for j in range(i + 1, m + 1):
            cost[i, j] = _chain_cost(seq, i, j)
    others = [k for k in range(1, m) if k != t]
    combos = np.array(list(combinations(others, count - 2)), dtype=np.int32).reshape(-1, count - 2)
    kept = np.concatenate(
        [np.zeros((len(combos), 1), np.int32), np.full((len(combos), 1), t, np.int32), combos], axis=1
    )
    kept.sort(axis=1)
    kept = np.concatenate([kept, np.full((len(kept), 1), m, np.int32)], axis=1)
    total = cost[kept[:, :-1], kept[:, 1:]].sum(axis=1)
    for row in np.argsort(total, kind="stable")[:20000]:
        idx = [(base + int(k)) % m for k in kept[row, :-1]]
        q = [poly[k] for k in idx]
        if not is_simple(q) or polygon_area(q) <= 0:
            continue
        if abs(polygon_area(q) / ref_area - 1.0) > tol:
            continue
        try:
            triangulate(q)
        except RuntimeError:
            continue
        return idx
    return None


def leaf_geometry(alpha, cell, max_points=REFERENCE_OUTLINE_POINTS, selection="dp"):
    """Outline, uv, triangles and aspect of the leaf in atlas cell `cell` (0..3).

    The reference outline has REFERENCE_OUTLINE_POINTS points. With a smaller `max_points` the full outline is
    simplified harder, as long as its area stays within outline_tolerance of the reference (otherwise
    the point count grows until it does, and the result says so in "points_needed"). `selection` is "dp"
    (Douglas-Peucker on the traced contour) or "optimal" (the best subset of the reference's vertices,
    see optimal_subset)."""
    ox, oy = (cell % 2) * CELL, (cell // 2) * CELL
    pad = 2
    a = np.pad(alpha[oy : oy + CELL, ox : ox + CELL], pad)
    loops = marching_squares(a)
    full = max(loops, key=len)
    # sample coordinates -> atlas texel-edge coordinates (a pixel centre sits at +0.5)
    full = full + np.array([ox - pad + 0.5, oy - pad + 0.5])
    xmin, ymin = full.min(axis=0)
    xmax, ymax = full.max(axis=0)

    def build(chosen):
        card = np.stack(
            [-1.0 + 2.0 * (chosen[:, 0] - xmin) / (xmax - xmin), 1.0 - 2.0 * (chosen[:, 1] - ymin) / (ymax - ymin)],
            axis=1,
        )
        uv = chosen / ATLAS
        poly = [tuple(p) for p in card]
        if polygon_area(poly) < 0:  # image y points down, so the card-space winding flips
            poly = poly[::-1]
            uv = uv[::-1]
        # round now so the validation sees what is written
        poly = [(round(x, 5), round(y, 5)) for x, y in poly]
        uv = [(round(float(u), 5), round(float(v), 5)) for u, v in uv]
        if not is_simple(poly):
            raise RuntimeError("rounded outline is not simple")
        return poly, uv

    chosen = simplify_to(full, REFERENCE_OUTLINE_POINTS)
    if chosen is None:
        raise RuntimeError("could not simplify the outline to a simple polygon")
    poly, uv = build(chosen)
    ref_poly, ref_uv = poly, uv
    points_needed = None
    if max_points < REFERENCE_OUTLINE_POINTS:
        ref_area = polygon_area(poly)
        tol = outline_tolerance(max_points)
        for limit in range(max_points, REFERENCE_OUTLINE_POINTS):
            if selection == "optimal":
                idx = optimal_subset(ref_poly, limit, tol, ref_area)
                if idx is None:
                    continue
                cpoly, cuv = [ref_poly[k] for k in idx], [ref_uv[k] for k in idx]
            else:
                cand = simplify_to(full, limit)
                if cand is None:
                    continue
                cpoly, cuv = build(cand)
            dev = abs(polygon_area(cpoly) / ref_area - 1.0)
            if dev <= tol:
                poly, uv, points_needed = cpoly, cuv, limit
                break
        else:
            points_needed = REFERENCE_OUTLINE_POINTS  # keeps the reference polygon
        dev = abs(polygon_area(poly) / ref_area - 1.0)
    else:
        dev = 0.0
    tris = triangulate(poly)
    area = polygon_area(poly)
    tri_area = sum(0.5 * _orient(poly[i], poly[j], poly[k]) for i, j, k in tris)
    if area <= 0 or abs(tri_area - area) > 0.005 * area:
        raise RuntimeError(f"triangulation area {tri_area} != polygon area {area}")
    if any(_orient(poly[i], poly[j], poly[k]) <= 0 for i, j, k in tris):
        raise RuntimeError("clockwise triangle")
    aspect = (xmax - xmin) / (ymax - ymin)
    coarse = coarse_outline(poly)
    return {
        "aspect": aspect,
        "outline": poly,
        "uv": uv,
        "triangles": [i for t in tris for i in t],
        "coarse_outline": [poly[i] for i in coarse["idx"]],
        "coarse_uv": [uv[i] for i in coarse["idx"]],
        "coarse_triangles": coarse["triangles"],
        "coarse_area_ratio": coarse["ratio"],
        "coarse_method": coarse["method"],
        "area_deviation": dev,
        "points_needed": points_needed,
        "reference_outline": ref_poly,
        "reference_uv": ref_uv,
        "sym_diff": sym_diff_ratio(ref_poly, poly) if poly is not ref_poly else 0.0,
    }


def srgb_to_linear(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def draw_overlay(path, color_img, geoms):
    """Debug picture: the atlas with each leaf's simplified outline (green) and triangles (blue)."""
    from PIL import ImageDraw

    scale = 2
    img = color_img.resize((ATLAS * scale, ATLAS * scale), Image.NEAREST).convert("RGB")
    d = ImageDraw.Draw(img)
    for g in geoms:
        uv = [(u * ATLAS * scale, v * ATLAS * scale) for u, v in g["uv"]]
        t = g["triangles"]
        for i in range(0, len(t), 3):
            d.polygon([uv[t[i]], uv[t[i + 1]], uv[t[i + 2]]], outline=(60, 120, 255))
        d.line(uv + [uv[0]], fill=(0, 255, 0), width=1)
        for p in uv:
            d.ellipse([p[0] - 2, p[1] - 2, p[0] + 2, p[1] + 2], fill=(255, 0, 0))
    img.save(path)


def draw_sheet(path, names, sets, color_images, title):
    """One contact sheet: a row per set, a tile per leaf, the reference outline (yellow) and the chosen
    polygon (cyan, vertices red) over the colour atlas, labelled with point counts and the symmetric
    difference to the reference."""
    from PIL import ImageDraw

    tile, head = 320, 22
    scale = tile / CELL
    sheet = Image.new("RGB", (4 * tile, len(names) * (tile + head) + head), (40, 40, 40))
    d = ImageDraw.Draw(sheet)
    d.text((6, 5), f"{title}: reference outline (yellow) vs chosen polygon (cyan)", fill=(255, 255, 255))
    for r, (name, st) in enumerate(zip(names, sets)):
        atlas = color_images[name]
        for k, g in enumerate(st["leaves"]):
            ox, oy = (k % 2) * CELL, (k // 2) * CELL
            x0, y0 = k * tile, head + r * (tile + head)
            sheet.paste(atlas.crop((ox, oy, ox + CELL, oy + CELL)).resize((tile, tile), Image.LANCZOS), (x0, y0 + head))

            def pt(u, v):
                return ((u * ATLAS - ox) * scale + x0, (v * ATLAS - oy) * scale + y0 + head)

            ref = [pt(u, v) for u, v in g["reference_uv"]]
            d.line(ref + [ref[0]], fill=(255, 230, 0), width=1)
            ch = [pt(u, v) for u, v in g["uv"]]
            d.line(ch + [ch[0]], fill=(0, 255, 255), width=2)
            for x, y in ch:
                d.ellipse([x - 3, y - 3, x + 3, y + 3], fill=(255, 0, 0))
            d.text(
                (x0 + 4, y0 + 5),
                f"{name} {k}: {len(g['outline'])}/{len(g['reference_outline'])} pts, symdiff {100 * g['sym_diff']:.1f}%",
                fill=(255, 255, 255),
            )
    sheet.save(path)


def fmt(v):
    s = f"{v:.5f}".rstrip("0")
    if s.endswith("."):
        s += "0"
    return "0.0" if s in ("-0.0",) else s


def pairs_rust(points, indent):
    items = [f"[{fmt(x)}, {fmt(y)}]" for x, y in points]
    lines = []
    for i in range(0, len(items), 4):
        lines.append(" " * indent + ", ".join(items[i : i + 4]) + ",")
    return "\n".join(lines)


def write_leaves_rs(path, names, sets, max_points, selection):
    leaves = [g for st in sets for g in st["leaves"]]
    n_full = [len(g["outline"]) for g in leaves]
    n_coarse = [len(g["coarse_outline"]) for g in leaves]
    tris = [len(g["triangles"]) // 3 for g in leaves]
    used = f"all {min(n_full)}" if min(n_full) == max(n_full) else f"{min(n_full)} to {max(n_full)}"
    out = [
        "//! Outlines, triangulations and atlas coordinates of the bundled leaf scans. Generated by",
        "//! `tools/prepare_textures.py`; do not edit.",
        f"// Full outlines: cap {max_points} points ({selection} selection), {used} used (mean {sum(n_full) / len(n_full):.1f}, "
        f"{sum(tris) / len(tris):.1f} triangles per card side); a leaf exceeds the cap only where it needs the points",
        f"// to stay within tolerance of the {REFERENCE_OUTLINE_POINTS}-point area. Coarse outlines: {min(n_coarse)} to {max(n_coarse)} points.",
        "// Outline points are measurements: one happens to be 0.63662, which clippy takes for 2/pi.",
        "#![allow(clippy::approx_constant)]",
        "",
        "/// One scanned leaf, in card space (x -1..1 across, y -1 at the base to 1 at the tip).",
        "pub(crate) struct ScannedLeaf {",
        "    /// Width over length of the leaf.",
        "    pub aspect: f32,",
        "    /// Outline points, counter-clockwise.",
        "    pub outline: &'static [[f32; 2]],",
        "    /// Atlas texture coordinate of each outline point (v = 0 at the top of the image).",
        "    pub uv: &'static [[f32; 2]],",
        "    /// Triangles over the outline points, three indices each, counter-clockwise.",
        "    pub triangles: &'static [u16],",
        "    /// A coarser outline (8 points, 10 for the few leaves that need them; a subset of `outline`) for distant levels of detail.",
        "    pub coarse_outline: &'static [[f32; 2]],",
        "    /// Atlas texture coordinate of each coarse outline point.",
        "    pub coarse_uv: &'static [[f32; 2]],",
        "    /// Triangles over the coarse outline points, three indices each, counter-clockwise.",
        "    pub coarse_triangles: &'static [u16],",
        "}",
        "",
        "/// The four leaves of one atlas, and its average colour.",
        "pub(crate) struct LeafSet {",
        "    /// The set's four leaves, in atlas cell order (left to right, top to bottom).",
        "    pub leaves: [ScannedLeaf; 4],",
        "    /// Average linear rgb of the leaves' pixels in the colour atlas.",
        "    pub mean: [f32; 3],",
        "}",
        "",
        f"// order: {', '.join(names)}",
        f"/// The sets in the order {', '.join(names)}.",
        f"pub(crate) const LEAF_SETS: [LeafSet; {len(sets)}] = [",
    ]
    for s in sets:
        out.append("    LeafSet {")
        out.append("        leaves: [")
        for g in s["leaves"]:
            out.append("            ScannedLeaf {")
            out.append(f"                aspect: {fmt(g['aspect'])},")
            out.append("                outline: &[")
            out.append(pairs_rust(g["outline"], 20))
            out.append("                ],")
            out.append("                uv: &[")
            out.append(pairs_rust(g["uv"], 20))
            out.append("                ],")
            tri = g["triangles"]
            out.append("                triangles: &[")
            for i in range(0, len(tri), 12):
                out.append(" " * 20 + ", ".join(str(t) for t in tri[i : i + 12]) + ",")
            out.append("                ],")
            out.append("                coarse_outline: &[")
            out.append(pairs_rust(g["coarse_outline"], 20))
            out.append("                ],")
            out.append("                coarse_uv: &[")
            out.append(pairs_rust(g["coarse_uv"], 20))
            out.append("                ],")
            out.append("                coarse_triangles: &[")
            ct = g["coarse_triangles"]
            for i in range(0, len(ct), 12):
                out.append(" " * 20 + ", ".join(str(t) for t in ct[i : i + 12]) + ",")
            out.append("                ],")
            out.append("            },")
        out.append("        ],")
        out.append(f"        mean: [{', '.join(fmt(m) for m in s['mean'])}],")
        out.append("    },")
    out.append("];")
    path.write_text("\n".join(out) + "\n")


def prepare_leaves(root, max_points=DEFAULT_OUTLINE_POINTS, out_rs=None, selection=DEFAULT_SELECTION, sheet=None):
    """Atlases and src/textures/leaves.rs. With `out_rs` (a variant run) the atlases go to a scratch
    directory and only that Rust file is written, so the worktree's own files stay untouched. `sheet` names a
    contact sheet PNG of reference vs chosen outlines."""
    variant = out_rs is not None
    out_dir = Path(f"/tmp/bevy_flora_leaf_variant_{max_points}{selection}") if variant else root / "src" / "textures" / "leaves"
    out_dir.mkdir(parents=True, exist_ok=True)
    for old in out_dir.glob("*.jpg"):
        old.unlink()
    DEBUG_DIR.mkdir(parents=True, exist_ok=True)

    print("== leaves: sources ==")
    built = []  # (name, set id, rgb, nrm, alpha)
    for name, candidates in LEAF_ROWS:
        for set_id in candidates:
            res = build_atlas(name, set_id)
            if res is not None:
                built.append((name, set_id, *res))
                if set_id != candidates[0]:
                    print(f"  !! SUBSTITUTION: {name} uses {set_id} instead of {candidates[0]}")
                break
            print(f"  !! {name}: {set_id} has touching leaves (fewer than 4 separate); skipped")
        else:
            print(f"  !! DROPPED: {name} has no usable leaf set")

    attempts = [
        ("quality 85/92, normals 512 px", 85, 92, 512),
        ("FALLBACK quality 80/88, normals 512 px", 80, 88, 512),
        ("FALLBACK quality 80/88, normals 256 px", 80, 88, 256),
    ]
    for label, qc, qn, nside in attempts:
        sizes = {}
        for name, _sid, rgb, nrm, _alpha in built:
            c_img = Image.fromarray(np.clip(rgb.round(), 0, 255).astype(np.uint8))
            n_img = Image.fromarray(np.clip(nrm.round(), 0, 255).astype(np.uint8))
            if nside != ATLAS:
                n_img = n_img.resize((nside, nside), Image.LANCZOS)
            for suffix, img, q in (("color", c_img, qc), ("normal", n_img, qn)):
                data = encode_jpeg(img, q)
                (out_dir / f"{name}_{suffix}.jpg").write_bytes(data)
                sizes[f"{name}_{suffix}.jpg"] = len(data)
        total = sum(sizes.values())
        print(f"== leaves: encode {label}: total {total} bytes ==")
        if total <= LEAF_BUDGET:
            break
    else:
        raise RuntimeError(f"leaf atlases are {total} bytes, over the {LEAF_BUDGET} budget")

    sets = []
    color_images = {}
    for name, set_id, _rgb, _nrm, alpha in built:
        color_img = Image.open(out_dir / f"{name}_color.jpg").convert("RGB")
        px = np.asarray(color_img, dtype=np.float64)
        mean = srgb_to_linear(px[alpha > 0.5]).mean(axis=0)
        geoms = [leaf_geometry(alpha, k, max_points, selection) for k in range(4)]
        color_images[name] = color_img
        draw_overlay(DEBUG_DIR / (f"{name}_overlay.png" if not variant else f"{name}_overlay_{max_points}{selection}.png"), color_img, geoms)
        sets.append({"leaves": geoms, "mean": [float(m) for m in mean]})
        print(f"  {name:15s} {set_id}")
        print(f"    files: {sizes[name + '_color.jpg']} B colour, {sizes[name + '_normal.jpg']} B normal")
        print(f"    outline points {[len(g['outline']) for g in geoms]}, aspects {[round(g['aspect'], 3) for g in geoms]}")
        print(
            f"    coarse points {[len(g['coarse_outline']) for g in geoms]}, "
            f"coarse/full area {[round(g['coarse_area_ratio'], 3) for g in geoms]}, "
            f"methods {[g['coarse_method'] for g in geoms]}"
        )
        if max_points < REFERENCE_OUTLINE_POINTS:
            print(
                f"    area deviation from the {REFERENCE_OUTLINE_POINTS}-point outline {[round(g['area_deviation'], 4) for g in geoms]}, "
                f"points needed {[g['points_needed'] for g in geoms]}"
            )
        print(f"    mean linear rgb {[round(m, 4) for m in sets[-1]['mean']]}")
    names = [b[0] for b in built]
    rs_path = Path(out_rs).resolve() if variant else root / "src" / "textures" / "leaves.rs"
    rs_path.parent.mkdir(parents=True, exist_ok=True)
    write_leaves_rs(rs_path, names, sets, max_points, selection)
    all_leaves = [g for st in sets for g in st["leaves"]]
    print(
        f"  full outline: mean {sum(len(g['outline']) for g in all_leaves) / len(all_leaves):.2f} points, "
        f"{sum(len(g['triangles']) // 3 for g in all_leaves) / len(all_leaves):.2f} triangles per leaf; "
        f"worst area deviation {max(g['area_deviation'] for g in all_leaves):.4f}"
    )
    for st, name in zip(sets, names):
        ls = st["leaves"]
        print(f"    {name:15s} triangles per leaf {[len(g['triangles']) // 3 for g in ls]} "
              f"(total {sum(len(g['triangles']) // 3 for g in ls)}), coarse {[len(g['coarse_triangles']) // 3 for g in ls]}")
        print(f"    {name:15s} mean outline points {sum(len(g['outline']) for g in ls) / 4:.2f}, "
              f"worst deviation {max(g['area_deviation'] for g in ls):.4f}, "
              f"symmetric difference worst {max(g['sym_diff'] for g in ls):.4f} mean {sum(g['sym_diff'] for g in ls) / 4:.4f}, "
              f"needed more than {max_points}: {sum(1 for g in ls if len(g['outline']) > max_points)}, "
              f"mean coarse points {sum(len(g['coarse_outline']) for g in ls) / 4:.2f}, coarse points {[len(g['coarse_outline']) for g in ls]}")
    print(
        f"  symmetric difference to the {REFERENCE_OUTLINE_POINTS}-point outline: worst {max(g['sym_diff'] for g in all_leaves):.4f}, "
        f"mean {sum(g['sym_diff'] for g in all_leaves) / len(all_leaves):.4f}; leaves above {max_points} points: "
        f"{sum(1 for g in all_leaves if len(g['outline']) > max_points)} of {len(all_leaves)}"
    )
    if sheet is not None:
        draw_sheet(Path(sheet), names, sets, color_images, f"{selection} {max_points}")
        print(f"wrote {sheet}")
    print(f"wrote {rs_path}; LEAF_SETS order: {', '.join(f'{i}={n}' for i, n in enumerate(names))}")
    print(f"  leaf total: {sum(sizes.values())} B (budget {LEAF_BUDGET}); used: {label}; overlays in {DEBUG_DIR}")


def leaf_credit_rows():
    rows = []
    for name, candidates in LEAF_ROWS:
        set_id = candidates[0]
        rows.append(
            {
                "files": f"leaves/{name}_color.jpg, leaves/{name}_normal.jpg",
                "asset": f"{set_id} ({'ambientCG'})",
                "author": "ambientCG",
                "url": f"https://ambientcg.com/a/{set_id}",
                "licence": "CC0 1.0",
                "tile": "n/a (leaf atlas)",
            }
        )
    return rows


# --------------------------------------------------------------------------
# Credits
# --------------------------------------------------------------------------


def write_credits(root, rows):
    lines = [
        "# Texture credits",
        "",
        "Every texture in this directory is released under CC0 1.0 (a public domain",
        "dedication): no attribution is required, but the sources and authors are",
        "credited here anyway. The files are downscaled and re-encoded copies of the",
        "originals, produced by `tools/prepare_textures.py`.",
        "",
        "| File(s) | Asset (id) | Author | Source | Licence | Tile (m) |",
        "|---|---|---|---|---|---|",
    ]
    for r in rows:
        lines.append(
            f"| {r['files']} | {r['asset']} | {r['author']} | <{r['url']}> | {r['licence']} | {r['tile']} |"
        )
    path = root / "src" / "textures" / "CREDITS.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")
    print(f"wrote {path} ({len(rows)} rows)")


STAGES = {"bark": prepare_bark, "leaves": prepare_leaves}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--only", choices=sorted(STAGES), help="run a single stage (default: all)")
    parser.add_argument(
        "--max-outline-points",
        type=int,
        default=DEFAULT_OUTLINE_POINTS,
        help=(
            f"leaves stage: cap on the full outline's points (default {DEFAULT_OUTLINE_POINTS}; a leaf may use "
            f"more to stay within 4%% (cap >= 16) / 8%% (cap >= 12) / 10%% (cap >= 10) of the "
            f"{REFERENCE_OUTLINE_POINTS}-point area; "
            f"other values need --out-rs)"
        ),
    )
    parser.add_argument(
        "--out-rs",
        type=Path,
        help="leaves stage: write the generated Rust here and leave every other file alone (default: src/textures/leaves.rs)",
    )
    parser.add_argument(
        "--selection",
        choices=("dp", "optimal"),
        default=DEFAULT_SELECTION,
        help="leaves stage: how the capped full outline is chosen: the best subset of the reference outline's "
        f"vertices, least symmetric-difference area (optimal, default {DEFAULT_SELECTION}), or Douglas-Peucker on "
        "the traced contour (dp); values other than the defaults need --out-rs",
    )
    parser.add_argument("--sheet", type=Path, help="leaves stage: write a contact sheet PNG of reference vs chosen outlines")
    args = parser.parse_args()
    if not 4 <= args.max_outline_points <= REFERENCE_OUTLINE_POINTS:
        parser.error(f"--max-outline-points must be between 4 and {REFERENCE_OUTLINE_POINTS}")
    variant = args.out_rs is not None
    if (args.max_outline_points != DEFAULT_OUTLINE_POINTS or args.selection != DEFAULT_SELECTION) and not variant:
        parser.error("--max-outline-points / --selection other than the default need --out-rs (the committed file keeps the default)")
    if variant and args.only != "leaves":
        parser.error("--out-rs / --max-outline-points need --only leaves")

    root = Path(__file__).resolve().parent.parent
    for stage, fn in STAGES.items():
        if args.only and args.only != stage:
            continue
        if stage == "leaves":
            fn(root, args.max_outline_points, args.out_rs, args.selection, args.sheet)
        else:
            fn(root)
    if variant:
        return 0
    # CREDITS.md always lists every stage, derived from the static tables, so a partial run keeps it whole.
    write_credits(root, bark_credit_rows() + leaf_credit_rows())
    return 0


if __name__ == "__main__":
    sys.exit(main())
