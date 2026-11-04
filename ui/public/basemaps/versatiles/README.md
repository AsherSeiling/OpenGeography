# VersaTiles Colorful — bundled style

Copied unchanged on 2026-11-04 from `https://tiles.versatiles.org/assets/styles/colorful/style.json` (versatiles-style 8.1.1;
the style's own metadata says CC0 / public domain, the generator code is MIT) together with its sprite sheet from
`https://tiles.versatiles.org/assets/sprites/base*` (icons from Maki and Temaki, CC0).

VersaTiles warns that the assets it hosts may change (renamed styles, different sprite names), so the app reads this copy.
Only the **tiles** (`https://tiles.versatiles.org/tiles/osm/…` and `…/landcover-vectors/…`) come from the network.
At run time the style's `sprite` is pointed at `sprites/base` here and its `glyphs` URL is not used: OpenLayers draws text
with the computer's own fonts, so no font files are bundled.
