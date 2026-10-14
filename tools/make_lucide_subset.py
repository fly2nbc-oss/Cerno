"""Cuts the Lucide icons Cerno uses out of the Lucide icon font.

Lucide (https://lucide.dev, ISC) ships every icon as a glyph of one TrueType font in the npm
package `lucide-static`. Cerno embeds only the icons it draws (`theme::icon`), as a subset of
that font: `assets/lucide/lucide-subset.ttf`, about 10 KB instead of 660 KB.

Get the package (its SHA-1 is the `shasum` the npm registry lists for the version):

    curl -LO https://registry.npmjs.org/lucide-static/-/lucide-static-0.460.0.tgz
    tar -xzf lucide-static-0.460.0.tgz package/font/lucide.ttf package/font/info.json package/LICENSE

then, with fontTools installed (`pip install fonttools`):

    python tools/make_lucide_subset.py <package dir> assets/lucide

Writes into <out dir>: `lucide-subset.ttf`, `LICENSE` (the package's ISC text) and
`README.md` with every icon's name and code point. Adding an icon: its name to ICONS, run the
script, the code point into `theme::icon` (`theme::tests::every_icon_is_in_the_subset` checks).
"""

import json
import shutil
import sys
from pathlib import Path

from fontTools import subset

VERSION = "0.460.0"
SHASUM = "3d6b70c74cc5e180ac773f684494eef770d95731"

# Lucide names, as in `info.json`.
ICONS = [
    "check",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "circle-help",
    "copy",
    "copy-check",
    "eye",
    "file-warning",
    "filter",
    "folder-open",
    "image",
    "layout-grid",
    "panel-bottom",
    "panel-left",
    "panel-right",
    "panel-top",
    "pause",
    "play",
    "rotate-ccw",
    "rotate-cw",
    "scan-face",
    "trash-2",
    "triangle-alert",
    "user",
    "user-x",
    "volume-1",
    "volume-2",
    "volume-x",
    "x",
]


def main(package: Path, out: Path) -> None:
    info = json.loads((package / "font" / "info.json").read_text(encoding="utf-8"))
    codes = {}
    for name in ICONS:
        entry = info.get(name)
        if entry is None:
            sys.exit(f"{name}: not in info.json of lucide-static {VERSION}")
        codes[name] = int(entry["encodedCode"].lstrip("\\"), 16)

    out.mkdir(parents=True, exist_ok=True)
    options = subset.Options()
    options.hinting = False
    options.layout_features = []
    options.name_IDs = ["*"]
    font = subset.load_font(str(package / "font" / "lucide.ttf"), options)
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=codes.values())
    subsetter.subset(font)
    subset.save_font(font, str(out / "lucide-subset.ttf"), options)
    shutil.copyfile(package / "LICENSE", out / "LICENSE")

    rows = "\n".join(f"| `{name}` | `U+{code:04X}` |" for name, code in codes.items())
    (out / "README.md").write_text(
        f"""# Lucide subset

`lucide-subset.ttf` holds the {len(codes)} [Lucide](https://lucide.dev) icons Cerno draws, cut
out of `lucide-static@{VERSION}/font/lucide.ttf` (npm, SHA-1 `{SHASUM}`) by
`tools/make_lucide_subset.py`. Licence: ISC, see [LICENSE](LICENSE).

| Icon | Code point |
|---|---|
{rows}
""",
        encoding="utf-8",
        newline="\n",
    )
    print(f"{len(codes)} icons -> {out / 'lucide-subset.ttf'}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(Path(sys.argv[1]), Path(sys.argv[2]))
