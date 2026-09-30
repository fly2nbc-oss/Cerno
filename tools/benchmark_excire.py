"""Compares Cerno's two aesthetic scores with Excire Foto's, photo by photo.

Cerno's index keeps LAION (CLIP ViT-L/14) and Aesthetic Predictor V2.5 (SigLIP) scores;
Excire Foto keeps its own aesthetics value in the catalogue's `backend.db` (plugin
`AestheticsTagger`, annotation `image_aesthetics`, column `confidence`). This script reads both
read-only, matches the photos by path, keeps one photo per Cerno fingerprint (exact duplicates
would inflate every correlation) and compares ranks only, since the three scales differ. The
user's own stars in Cerno's index are the one reference that is not a model.

Excire must be closed: its catalogue is a live WAL database. Without --excire-db the catalogue
is the one Excire had open last (`CurrentDatabasePath` in its global settings). Every Excire
table is read without its indexes (`NOT INDEXED`): a catalogue with broken indexes can still be
scanned.

The outputs name and show the photos, so they belong outside the repository:
per_photo.csv, summary.md, disagreements.html.

    python tools/benchmark_excire.py --photos <folder> --out <folder>
        [--excire-db <backend.db>] [--cerno-db <cerno.db>]
"""

import argparse
import base64
import collections
import contextlib
import csv
import html
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import urllib.parse
import urllib.request

import numpy as np
from scipy import stats

BOOTSTRAP = 2000
PERMUTATIONS = 1000
SEED = 20260930
# `view::SERIES_GAP_MS`: photos of one camera at most 2 s apart form a series.
SERIES_GAP_MS = 2000
# Share of the photos that counts as "best" and "worst" for the tail overlap.
TAIL = 0.10
# Disagreement sheet: photos per model and direction.
SHEET_ROWS = 15
# Excire values outside this range are not aesthetics scores (damaged rows).
EXCIRE_RANGE = (0.0, 10.0)
# Excire's text recognition (`ocr_texts`): a photo with at least TEXT_CHARS characters read
# with at least OCR_CONFIDENCE counts as a document (slide, screen, screenshot, notes).
OCR_CONFIDENCE = 0.8
TEXT_CHARS = 50


def key(path):
    return os.path.normcase(os.path.normpath(str(path)))


def connect_ro(path):
    """Opens a database without ever writing to it or its folder.

    With a -wal file present the database may have unmerged pages, so it is opened read-only
    and SQLite reads the log. Without one it is opened as immutable: a plain read-only
    connection would create empty -wal and -shm files beside it.
    """
    path = pathlib.Path(path).resolve()
    if not path.is_file():
        sys.exit(f"no database at {path}")
    wal = pathlib.Path(f"{path}-wal")
    mode = "mode=ro" if wal.exists() else "mode=ro&immutable=1"
    return contextlib.closing(sqlite3.connect(f"{path.as_uri()}?{mode}", uri=True))


def stamps(*paths):
    """Size and modification time of each database and its -wal / -shm files."""
    found = {}
    for path in paths:
        for suffix in ("", "-wal", "-shm"):
            file = pathlib.Path(f"{path}{suffix}")
            if file.exists():
                status = file.stat()
                found[str(file)] = (status.st_size, status.st_mtime_ns)
    return found


def excire_running():
    if os.name == "nt":
        listing = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True)
        return '"excire foto.exe"' in listing.stdout.decode(errors="replace").lower()
    return subprocess.run(["pgrep", "-if", "excire foto"], capture_output=True).returncode == 0


def current_excire_catalogue():
    settings = pathlib.Path(os.environ.get("APPDATA", ""), "Excire Foto", "excire_foto.global.db")
    with connect_ro(settings) as db:
        row = db.execute(
            "SELECT Value FROM settings_entry NOT INDEXED WHERE Key = 'CurrentDatabasePath'"
        ).fetchone()
    if row is None:
        sys.exit(f"{settings} names no current catalogue; pass --excire-db")
    return pathlib.Path(json.loads(row[0]), "backend.db"), settings


def uri_to_path(uri):
    return urllib.request.url2pathname(urllib.parse.urlparse(uri).path)


def read_excire(path):
    with connect_ro(path) as db:
        check = [row[0] for row in db.execute("PRAGMA quick_check(5)")]
        plugin = db.execute(
            "SELECT id FROM plugins NOT INDEXED WHERE plugin_name = 'AestheticsTagger'"
        ).fetchone()
        annotation = db.execute(
            "SELECT id FROM annotations NOT INDEXED WHERE annotation = 'image_aesthetics'"
        ).fetchone()
        if plugin is None or annotation is None:
            sys.exit(f"{path} has no AestheticsTagger / image_aesthetics – another Excire version?")
        rows = db.execute(
            "SELECT image_id, region_id, confidence FROM annotation_relations NOT INDEXED "
            "WHERE plugin_id = ? AND annotation_id = ?",
            (plugin[0], annotation[0]),
        ).fetchall()
        images = db.execute("SELECT id, file_uri FROM images NOT INDEXED").fetchall()
        failed = db.execute("SELECT count(*) FROM error_images NOT INDEXED").fetchone()[0]
        try:
            ocr = db.execute(
                "SELECT image_id, length(trim(text)) FROM ocr_texts NOT INDEXED WHERE confidence >= ?",
                (OCR_CONFIDENCE,),
            ).fetchall()
        except sqlite3.OperationalError:
            ocr = None

    per_image = collections.defaultdict(set)
    out_of_range = 0
    for image_id, region, value in rows:
        if value is None or not EXCIRE_RANGE[0] <= value <= EXCIRE_RANGE[1]:
            out_of_range += 1
            continue
        per_image[image_id].add((region, value))

    characters = collections.Counter()
    for image_id, length in ocr or ():
        characters[image_id] += length

    values, paths, text = {}, {}, {}
    conflicting = 0
    for image_id, uri in images:
        path = uri_to_path(uri)
        paths[key(path)] = path
        if ocr is not None:
            text[key(path)] = characters[image_id]
        found = per_image.get(image_id)
        if not found:
            continue
        if len({value for _, value in found}) > 1:
            conflicting += 1
        values[key(path)] = min(found)[1]
    facts = {
        "quick_check": check,
        "rows": len(rows),
        "distinct_rows": sum(len(found) for found in per_image.values()),
        "out_of_range": out_of_range,
        "conflicting": conflicting,
        "images": len(images),
        "with_value": len(values),
        "failed": failed,
    }
    return values, paths, text, facts


def not_a_photo(folder):
    """Cerno's kept originals, and Excire's preview caches when its catalogue sits in the folder."""
    folder = folder.lower()
    return folder == ".originals" or folder.endswith(".exfoto")


def read_cerno(path, root):
    root = pathlib.Path(root).resolve()
    prefix = key(root) + os.sep
    with connect_ro(path) as db:
        rows = db.execute(
            "SELECT f.path, f.fingerprint, f.rating, i.aesthetic, i.aesthetic25, i.taken_ms, "
            "i.camera, i.thumbnail FROM files f LEFT JOIN images i ON i.fingerprint = f.fingerprint "
            "WHERE f.path LIKE ?",
            (str(root) + "%",),
        ).fetchall()
    photos, gone = [], 0
    for file, fingerprint, rating, laion, v25, taken, camera, thumbnail in rows:
        file = pathlib.Path(file)
        if not key(file).startswith(prefix) or any(not_a_photo(part) for part in file.parts):
            continue
        if not file.is_file():
            gone += 1
            continue
        relative = file.relative_to(root)
        photos.append({
            "path": file,
            "key": key(file),
            "relative": relative.as_posix(),
            "folder": relative.parts[0] if len(relative.parts) > 1 else ".",
            "fingerprint": fingerprint,
            # A rejected photo (-1) counts as 0 stars, as in the For you model.
            "stars": None if rating is None else max(rating, 0),
            "laion": laion,
            "v25": v25,
            "taken": taken,
            "camera": camera,
            "thumbnail": thumbnail,
            "edited": (file.parent / ".originals" / file.name).exists(),
        })
    return photos, gone


def one_per_fingerprint(photos, excire):
    groups = collections.defaultdict(list)
    for photo in photos:
        groups[photo["fingerprint"]].append(photo)
    unique, spread = [], []
    for members in groups.values():
        members.sort(key=lambda photo: photo["relative"])
        values = [excire[photo["key"]] for photo in members]
        spread.append(max(values) - min(values))
        first = dict(members[0])
        first["copies"] = len(members)
        first["excire"] = values[0]
        first["edited"] = any(photo["edited"] for photo in members)
        first["stars"] = next((p["stars"] for p in members if p["stars"] is not None), None)
        unique.append(first)
    unique.sort(key=lambda photo: photo["relative"])
    return unique, spread


def percentile_ranks(values):
    return (stats.rankdata(values) - 0.5) / len(values)


def spearman_rows(a, b):
    """Spearman's rho for each row pair of two (runs, n) arrays."""
    ra = stats.rankdata(a, axis=1)
    rb = stats.rankdata(b, axis=1)
    ra -= ra.mean(axis=1, keepdims=True)
    rb -= rb.mean(axis=1, keepdims=True)
    # A resample of few photos can hold one value only; its rho is NaN and left out.
    with np.errstate(invalid="ignore", divide="ignore"):
        return (ra * rb).sum(axis=1) / np.sqrt((ra * ra).sum(axis=1) * (rb * rb).sum(axis=1))


def interval(samples):
    low, high = np.nanpercentile(samples, [2.5, 97.5])
    return float(low), float(high)


class Rho:
    def __init__(self, x, y, draws):
        self.value = float(stats.spearmanr(x, y).statistic)
        self.samples = spearman_rows(x[draws], y[draws])
        self.low, self.high = interval(self.samples)

    def __str__(self):
        return f"{self.value:.3f} [{self.low:.3f}, {self.high:.3f}]"


def tail_overlap(model, reference, share):
    count = max(1, round(share * len(model)))
    top = lambda values: set(np.argsort(-values, kind="stable")[:count])
    bottom = lambda values: set(np.argsort(values, kind="stable")[:count])
    return (
        len(top(model) & top(reference)) / count,
        len(bottom(model) & bottom(reference)) / count,
    )


def quintile_agreement(model, reference):
    a = np.minimum((percentile_ranks(model) * 5).astype(int), 4)
    b = np.minimum((percentile_ranks(reference) * 5).astype(int), 4)
    observed = np.zeros((5, 5))
    np.add.at(observed, (a, b), 1)
    observed /= observed.sum()
    expected = np.outer(observed.sum(axis=1), observed.sum(axis=0))
    weights = (np.subtract.outer(np.arange(5), np.arange(5)) ** 2) / 16
    kappa = 1 - (weights * observed).sum() / (weights * expected).sum()
    return float(np.mean(a == b)), float(np.mean(abs(a - b) <= 1)), float(kappa)


def series_groups(photos):
    """`view::series_places` on the unique photos: indices of each series of two or more."""
    timed = sorted(
        (index for index, photo in enumerate(photos) if photo["taken"] is not None),
        key=lambda i: (photos[i]["camera"] is not None, photos[i]["camera"] or "", photos[i]["taken"], i),
    )
    groups = []
    for index in timed:
        if groups:
            previous = photos[groups[-1][-1]]
            if (
                previous["camera"] == photos[index]["camera"]
                and photos[index]["taken"] - previous["taken"] <= SERIES_GAP_MS
            ):
                groups[-1].append(index)
                continue
        groups.append([index])
    return [group for group in groups if len(group) > 1]


def series_hits(groups, model, reference):
    hits = [
        group[int(np.argmax(model[group]))] == group[int(np.argmax(reference[group]))]
        for group in groups
    ]
    return float(np.mean(hits)) if hits else float("nan")


def stats_row(name, values):
    q = np.percentile(values, [0, 10, 50, 90, 100])
    return (
        f"| {name} | {q[0]:.2f} | {q[1]:.2f} | {q[2]:.2f} | {q[3]:.2f} | {q[4]:.2f} "
        f"| {values.mean():.2f} | {values.std(ddof=1):.2f} |"
    )


def write_csv(path, photos, pct, series_of):
    with open(path, "w", newline="", encoding="utf-8") as file:
        out = csv.writer(file)
        out.writerow([
            "path", "fingerprint", "copies", "edited", "folder", "excire", "laion", "v25",
            "excire_pct", "laion_pct", "v25_pct", "stars", "series", "series_size", "text_chars",
        ])
        for i, photo in enumerate(photos):
            series, size = series_of.get(i, ("", ""))
            out.writerow([
                photo["relative"], photo["fingerprint"], photo["copies"], int(photo["edited"]),
                photo["folder"], f"{photo['excire']:.4f}", f"{photo['laion']:.4f}",
                f"{photo['v25']:.4f}", f"{pct['E'][i]:.4f}", f"{pct['L'][i]:.4f}",
                f"{pct['V'][i]:.4f}", "" if photo["stars"] is None else photo["stars"],
                series, size, "" if photo["text"] is None else photo["text"],
            ])


SHEET_STYLE = """
body { margin: 0; padding: 24px 16px; background: #141414; color: #E6E6E6;
       font: 13px/1.4 "Segoe UI", system-ui, sans-serif; }
h1 { font-size: 22px; margin: 0 0 4px; } h2 { font-size: 17px; margin: 32px 0 4px; }
p { color: #9A9A9A; margin: 0 0 12px; max-width: 70em; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
figure { margin: 0; background: #202020; border-radius: 6px; overflow: hidden; }
img { display: block; width: 100%; height: 160px; object-fit: contain; background: #161616; }
figcaption { padding: 8px; font-size: 12px; }
.name { color: #9A9A9A; overflow-wrap: anywhere; }
.values { font-variant-numeric: tabular-nums; }
.gap { color: #5B8EC4; }
"""


def write_sheet(path, photos, pct):
    names = {"L": "LAION", "V": "V2.5"}
    parts = [
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>",
        "<meta name='viewport' content='width=device-width, initial-scale=1'>",
        f"<title>Excire disagreements</title><style>{SHEET_STYLE}</style></head><body>",
        "<h1>Where Cerno and Excire disagree most</h1>",
        "<p>Values are percentile ranks within this folder (0 = worst, 100 = best), raw scores in "
        "brackets. One photo per fingerprint.</p>",
    ]
    for model in ("L", "V"):
        gap = pct[model] - pct["E"]
        for title, order in (
            (f"{names[model]} ranks higher than Excire", np.argsort(-gap, kind="stable")),
            (f"Excire ranks higher than {names[model]}", np.argsort(gap, kind="stable")),
        ):
            parts.append(f"<h2>{html.escape(title)}</h2><div class='grid'>")
            for i in order[:SHEET_ROWS]:
                photo = photos[i]
                image = ""
                if photo["thumbnail"]:
                    data = base64.b64encode(photo["thumbnail"]).decode()
                    image = f"<img alt='' src='data:image/jpeg;base64,{data}'>"
                stars = "" if photo["stars"] is None else f" · stars {photo['stars']}"
                parts.append(
                    f"<figure>{image}<figcaption><div class='name'>{html.escape(photo['relative'])}</div>"
                    f"<div class='values'>Excire {pct['E'][i] * 100:.0f} ({photo['excire']:.2f}) · "
                    f"L {pct['L'][i] * 100:.0f} ({photo['laion']:.2f}) · "
                    f"V {pct['V'][i] * 100:.0f} ({photo['v25']:.2f}){stars}</div>"
                    f"<div class='gap'>gap {gap[i] * 100:+.0f}</div></figcaption></figure>"
                )
            parts.append("</div>")
    parts.append("</body></html>")
    pathlib.Path(path).write_text("".join(parts), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--photos", required=True, help="folder both programs analysed")
    parser.add_argument("--out", required=True, help="folder for the results (created)")
    parser.add_argument("--excire-db", help="Excire's backend.db (default: its current catalogue)")
    parser.add_argument("--cerno-db", help="Cerno's index (default: %%LOCALAPPDATA%%\\Cerno\\data\\cerno.db)")
    parser.add_argument(
        "--allow-running-excire", action="store_true",
        help="only for a catalogue Excire does not have open, e.g. a test database",
    )
    args = parser.parse_args()
    sys.stdout.reconfigure(errors="replace")

    if args.allow_running_excire and not args.excire_db:
        sys.exit("--allow-running-excire needs --excire-db")
    if not args.allow_running_excire and excire_running():
        sys.exit("Excire Foto is running – close it first, its catalogue is a live database")

    watched = []
    if args.excire_db:
        excire_db = pathlib.Path(args.excire_db)
    else:
        excire_db, settings = current_excire_catalogue()
        watched.append(settings)
    cerno_db = pathlib.Path(
        args.cerno_db or pathlib.Path(os.environ.get("LOCALAPPDATA", ""), "Cerno", "data", "cerno.db")
    )
    watched += [excire_db, cerno_db]
    before = stamps(*watched)

    excire, excire_paths, text, facts = read_excire(excire_db)
    photos, gone = read_cerno(cerno_db, args.photos)
    root = key(pathlib.Path(args.photos).resolve()) + os.sep
    excire_here = {k for k in excire_paths if k.startswith(root)}
    excire_valued = {k for k in excire if k.startswith(root)}

    scored = [p for p in photos if p["laion"] is not None and p["v25"] is not None]
    matched = [p for p in scored if p["key"] in excire]
    cerno_keys = {p["key"] for p in photos}
    without_value = sorted(excire_paths[k] for k in excire_here - excire_valued if k in cerno_keys)
    only_cerno = sorted(p["relative"] for p in scored if p["key"] not in excire_here)
    only_excire = sorted(excire_paths[k] for k in excire_here - cerno_keys)

    unique, spread = one_per_fingerprint(matched, excire)
    for photo in unique:
        photo["text"] = text.get(photo["key"])
    n = len(unique)
    if n < 10:
        sys.exit(f"only {n} photos with all three scores – wrong --photos folder?")
    E = np.array([p["excire"] for p in unique])
    L = np.array([p["laion"] for p in unique])
    V = np.array([p["v25"] for p in unique])
    pct = {"E": percentile_ranks(E), "L": percentile_ranks(L), "V": percentile_ranks(V)}
    C = (pct["L"] + pct["V"]) / 2
    models = {"LAION": L, "V2.5": V, "L+V (mean rank)": C}

    rng = np.random.default_rng(SEED)
    draws = rng.integers(0, n, size=(BOOTSTRAP, n))
    to_excire = {name: Rho(values, E, draws) for name, values in models.items()}
    kendall = {name: float(stats.kendalltau(values, E).statistic) for name, values in models.items()}
    between = Rho(L, V, draws)
    shuffled = np.array([rng.permutation(n) for _ in range(PERMUTATIONS)])
    null = {
        name: spearman_rows(E[shuffled], np.broadcast_to(values, (PERMUTATIONS, n)))
        for name, values in (("LAION", L), ("V2.5", V))
    }

    groups = series_groups(unique)
    series_of = {i: (number, len(group)) for number, group in enumerate(groups, 1) for i in group}

    rated = np.array([p["stars"] is not None for p in unique])
    stars = np.array([p["stars"] if p["stars"] is not None else np.nan for p in unique])
    rated_draws = rng.integers(0, int(rated.sum()), size=(BOOTSTRAP, int(rated.sum())))

    lines = [
        "# Aesthetics benchmark: Cerno against Excire Foto",
        "",
        f"Photos: `{args.photos}`  ",
        f"Excire catalogue: `{excire_db}`  ",
        f"Cerno index: `{cerno_db}`",
        "",
        "## Counts",
        "",
        f"- Excire quick_check: {'ok' if facts['quick_check'] == ['ok'] else 'DAMAGED – ' + ' / '.join(facts['quick_check'])[:400]}",
        f"- Excire aesthetics rows {facts['rows']} (distinct {facts['distinct_rows']}, out of range "
        f"{facts['out_of_range']}, images with differing values {facts['conflicting']})",
        f"- Excire images {facts['images']} (in this folder {len(excire_here)}), with a value "
        f"{len(excire_valued)}, failed to analyse (videos …) {facts['failed']}",
        f"- Cerno files in this folder {len(photos)} (index rows without a file: {gone}), with both "
        f"scores {len(scored)}",
        f"- Matched {len(matched)} → **{n} unique photos** (one per fingerprint; "
        f"{len(matched) - n} exact duplicates dropped)",
        f"- Duplicates with differing Excire values: {sum(1 for s in spread if s > 1e-4)}"
        f" (largest difference {max(spread):.4f})",
        f"- In Excire without a value: {len(without_value)}"
        + "".join(f"\n  - `{pathlib.Path(p).name}`" for p in without_value[:20]),
        f"- Only in Cerno: {len(only_cerno)}" + "".join(f"\n  - `{p}`" for p in only_cerno[:20]),
        f"- Only in Excire: {len(only_excire)}"
        + "".join(f"\n  - `{pathlib.Path(p).name}`" for p in only_excire[:20]),
        "",
        "## Score distributions (unique photos)",
        "",
        "| Score | min | p10 | median | p90 | max | mean | sd |",
        "|---|---|---|---|---|---|---|---|",
        stats_row("Excire", E),
        stats_row("LAION", L),
        stats_row("V2.5", V),
        "",
        "## Agreement with Excire (ranks)",
        "",
        f"Spearman ρ with bootstrap 95 % interval ({BOOTSTRAP} resamples), Kendall τ. "
        f"Top / bottom: share of Excire's best / worst {TAIL:.0%} that the model puts there too "
        f"(chance: {TAIL:.0%}). Quintiles: each score split into five equal groups; exact, ±1, "
        "quadratic weighted κ.",
        "",
        "| Model | ρ [95 %] | τ | top | bottom | quintile exact | ±1 | κ |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for name, values in models.items():
        top, bottom = tail_overlap(values, E, TAIL)
        exact, near, kappa = quintile_agreement(values, E)
        lines.append(
            f"| {name} | {to_excire[name]} | {kendall[name]:.3f} | {top:.0%} | {bottom:.0%} "
            f"| {exact:.0%} | {near:.0%} | {kappa:.3f} |"
        )
    lines.append("")
    for first, second in (("LAION", "V2.5"), ("L+V (mean rank)", "V2.5"), ("L+V (mean rank)", "LAION")):
        low, high = interval(to_excire[first].samples - to_excire[second].samples)
        lines.append(
            f"- ρ({first}) − ρ({second}) = "
            f"{to_excire[first].value - to_excire[second].value:+.3f} [{low:+.3f}, {high:+.3f}]"
        )
    lines += [
        "  (paired bootstrap; an interval that contains 0 means no clear winner)",
        f"- LAION against V2.5: ρ = {between}",
        "- Shuffled Excire values (sanity check, "
        f"{PERMUTATIONS} permutations): mean ρ "
        + ", ".join(f"{name} {np.mean(r):+.3f}, 99th percentile of |ρ| {np.percentile(abs(r), 99):.3f}" for name, r in null.items()),
        "",
        "## Series",
        "",
        f"{len(groups)} series (at most {SERIES_GAP_MS} ms apart, same camera) with "
        f"{sum(len(g) for g in groups)} photos. Share of series where the model's best photo is "
        f"Excire's best; chance {np.mean([1 / len(g) for g in groups]) if groups else float('nan'):.0%}.",
        "",
        "| Model | same best photo |",
        "|---|---|",
    ]
    for name, values in models.items():
        lines.append(f"| {name} | {series_hits(groups, values, E):.0%} |")

    lines += [
        "",
        "## Against the user's own stars",
        "",
        f"{int(rated.sum())} unique photos with stars in Cerno (rejected = 0); stars "
        + ", ".join(f"{s}: {int(np.sum(stars == s))}" for s in range(6))
        + ". Spearman ρ with bootstrap 95 % interval – with this few photos the intervals are wide.",
        "",
        "| Score | ρ with stars [95 %] |",
        "|---|---|",
    ]
    if rated.sum() >= 10:
        for name, values in {"Excire": E, **models}.items():
            lines.append(f"| {name} | {Rho(values[rated], stars[rated], rated_draws)} |")
    else:
        lines.append("| – | fewer than 10 rated photos |")

    lines += [
        "",
        "## By folder",
        "",
        "| Folder | photos | ρ LAION | ρ V2.5 | ρ L+V |",
        "|---|---|---|---|---|",
    ]
    folders = sorted({p["folder"] for p in unique})
    for folder in folders:
        mask = np.array([p["folder"] == folder for p in unique])
        if mask.sum() < 10:
            lines.append(f"| {folder} | {int(mask.sum())} | – | – | – |")
            continue
        rhos = [stats.spearmanr(values[mask], E[mask]).statistic for values in models.values()]
        lines.append(f"| {folder} | {int(mask.sum())} | " + " | ".join(f"{r:.3f}" for r in rhos) + " |")
    kept = np.array([not p["edited"] for p in unique])
    lines += [
        "",
        f"Without the {int((~kept).sum())} photos Cerno edited (original in `.originals`; Excire "
        "may have scored the original): "
        + ", ".join(
            f"{name} ρ = {stats.spearmanr(values[kept], E[kept]).statistic:.3f}"
            for name, values in models.items()
        ),
    ]

    if text:
        characters = np.array([photo["text"] or 0 for photo in unique])
        documents = characters >= TEXT_CHARS
        photos_only = ~documents
        count = int(photos_only.sum())
        photo_draws = rng.integers(0, count, size=(BOOTSTRAP, count))
        lines += [
            "",
            "## Documents",
            "",
            f"{int(documents.sum())} photos with at least {TEXT_CHARS} characters of text that "
            f"Excire's text recognition read with confidence ≥ {OCR_CONFIDENCE} (slides, screens, "
            "notes, screenshots …). Their mean percentile rank (50 = average): "
            + ", ".join(
                f"{name} {pct[k][documents].mean() * 100:.0f}"
                for name, k in (("Excire", "E"), ("LAION", "L"), ("V2.5", "V"))
            )
            + ".",
            "",
            f"| Model | ρ with Excire without documents ({count} photos) [95 %] |",
            "|---|---|",
        ]
        for name, values in models.items():
            lines.append(f"| {name} | {Rho(values[photos_only], E[photos_only], photo_draws)} |")

    lines += [
        "",
        "## Spot check in Excire",
        "",
        "Open these in Excire and compare the aesthetics value it shows with the catalogue value:",
        "",
    ]
    order = np.argsort(E, kind="stable")
    for share in (0.1, 0.5, 0.9):
        photo = unique[order[int(share * (n - 1))]]
        lines.append(
            f"- `{photo['relative']}`: Excire {photo['excire']:.4f} (LAION {photo['laion']:.2f}, "
            f"V2.5 {photo['v25']:.2f})"
        )

    after = stamps(*watched)
    changed = sorted(set(before) ^ set(after) | {f for f in before if before[f] != after.get(f)})
    lines += [
        "",
        "## Read-only check",
        "",
        "Databases unchanged." if not changed else "CHANGED during the run: " + ", ".join(changed),
        "",
    ]

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "summary.md").write_text("\n".join(lines), encoding="utf-8")
    write_csv(out / "per_photo.csv", unique, pct, series_of)
    write_sheet(out / "disagreements.html", unique, pct)

    print(f"{n} unique photos compared ({len(matched)} matched, {len(without_value)} without an Excire value)")
    for name in models:
        print(f"  Spearman with Excire, {name}: {to_excire[name]}")
    print(f"  LAION against V2.5: {between}")
    print("databases unchanged" if not changed else f"WARNING, changed: {changed}")
    print(f"results in {out.resolve()}")


if __name__ == "__main__":
    main()
