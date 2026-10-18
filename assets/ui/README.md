# B18N illustrated editor assets

The five JPEG files are the original user-supplied illustrations, copied unchanged.
`head-reference.jpg` records the original control arrangement; it is not embedded in the plugin.
The editor crops the originals at draw time. Circular dial meshes sample the central knob,
leaving the scale fixed. The toggle uses a stationary nut and an animated lever.

`Libron-Regular.ttf` and `Libron-Bold.ttf`: unmodified Libron v0.31 by Nico Verbruggen,
SIL Open Font License 1.1. See `Libron-LICENSE.txt` for complete attribution, including
Newsreader and Readerly. Release: https://github.com/nicoverbruggen/libron/releases/tag/v0.31 .
Download: https://github.com/nicoverbruggen/libron/releases/download/v0.31/Libron.zip .
Regular is used for the controls and guide; Bold is used for B18N and the logo wordmark.
Both fonts are embedded and require no system installation. The full license and source
link are also available inside the editor's settings.

`manual.png`: generated with the built-in imagegen tool using `knob.jpg` as a style reference.
Prompt: Create a small open user-manual book icon for a vintage amplifier plugin.
Match the supplied reference image's hand-painted watercolor and dark ink outline style,
charcoal navy cover, warm ivory pages, subtle pale blue shadows. Single centered open book,
no text, no letters, clean silhouette, transparent background, square canvas.
This is a UI icon readable at 40 pixels.

The UI does not invent power, standby, or ground parameters. CAB toggles the existing
speaker parameter between Direct and the last selected 18/15 profile (18 on first enable).
The remembered choice is editor-local; the current audible speaker value remains host-owned.
Rust Pluggins is a left-column logo lockup; no distribution URL has been supplied.

`cabinet-cutout.png` is a transparent derivative generated from `cabinet.jpg`
with the built-in imagegen tool. The original remains unchanged.
Prompt: Background extraction only. Remove the peach background from this exact
watercolor amplifier and cabinet image. Preserve the original amplifier, cabinet,
wheels, grille, head, dark outlines, all original proportions and watercolor details.
Do not redesign, redraw, add or remove any amplifier part. Return the same complete
upright amplifier centered at the same scale and position on a genuinely transparent
background, preserving the original 3:4 aspect ratio.

`logo-reference.png`: original user-supplied reference, unchanged.
`rust-pluggins-emblem.png`: original white emblem generated with built-in imagegen.
The wordmark is rendered separately in embedded Libron Bold for exact font matching.
Prompt: Create an original logo emblem for an audio plugin publisher called Rust Pluggins.
The supplied transparent black reference is a rounded square enclosing bold concentric circular
audio-wave cutouts. Use it only as a composition reference: make a distinct new mark with a bold
rounded-square outline enclosing three circular arcs and a simple audio plug motif integrated
into the arcs. Minimal geometric modern mark, thick confident strokes, legible at 64 pixels.
Pure WHITE ink only on genuinely transparent background, no black, no shadows, no gradients,
NO TEXT, no letters, no wordmark. Center the emblem and fill 85 percent of square canvas.
It will be placed above a separately rendered Libron Bold wordmark on pastel orange.

Palette: left pastel orange #FFC196 (sampled from the original cabinet image), white branding;
right sky blue #9FC0D5, deep navy text #142B41. Right illustration and controls scale by 1.35.

`connected-jack.png`: watercolor jack/cable overlay generated with built-in imagegen,
using `head-reference.jpg` as a style reference. A single shared overlay is anchored
at the selected socket for each of the three input states.
Prompt: Create a single UI overlay asset: a male 1/4-inch instrument jack plug already
inserted into a front-facing amplifier socket, so NO exposed metal tip is visible.
Only the silver metal collar and barrel of the cable plug and the dark charcoal cable
are visible. Same hand-painted watercolor with fine dark ink outlines as the attached
amplifier reference, understated vintage equipment illustration. Transparent background,
no text, no labels, no amplifier, no socket panel. Portrait 3:4 composition. The small
circular attachment face of the plug is near the TOP RIGHT, at 82% canvas width and 10%
canvas height, with the silver barrel extending diagonally down-left, and the black cable
gently curving down toward the BOTTOM LEFT edge. Keep the plug slim, natural metal
highlights, clean silhouette. This asset will be anchored by its top-right attachment
face onto one of three selectable amplifier inputs at 90 by 120 display pixels.

Cable correction: `connected-jack.png` was edited with built-in imagegen to remove
all female socket hardware (hexagonal nut, washer and concentric mounting rings).
Only the male plug barrel, strain relief and cable remain. The existing socket
in `head.jpg` supplies the amplifier input in all three selected-input states.
Edit prompt: Remove the entire amplifier female socket assembly at the top right.
Leave only the male cable plug's slim silver cylindrical barrel, black strain relief
and trailing cable. Preserve the watercolor rendering, cable curve, diagonal orientation,
canvas dimensions and barrel/cable positions. No exposed long tip, washer, nut, socket
or surrounding circular base; removed hardware must be genuinely transparent.
