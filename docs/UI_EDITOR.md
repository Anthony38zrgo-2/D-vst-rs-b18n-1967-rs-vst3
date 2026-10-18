# B18N illustrated editor

Implemented 2026-10-18. Native NIH-plug/egui editor at 1280 x 744 logical pixels.

- Pastel-orange left column: white Libron Bold B18N title, cabinet, white emblem and Rust Pluggins wordmark.
- Original blank head illustration with six animated illustrated knobs.
- Clickable input jacks and Bass/Guitar/Instrument selector.
- 18/15-inch approximate speaker profiles, animated CAB toggle and Direct/off state.
- Input/output trim, host bypass, quality and experimental load selection.
- Illustrated manual icon opens a scrollable user guide with no circuit/algorithm terminology.
- Embedded Libron v0.31 Regular and Bold fonts. Asset provenance, generation prompts and font license are in assets/ui.

The editor writes through ParamSetter, including begin/end automation gestures.
Parameter IDs, enum order, VST3 identity, defaults and production DSP are unchanged by this UI work.
Existing unrelated working-tree changes were retained, including the multirate DSP algorithm version.
No distribution URL was supplied; the Rust Pluggins logo currently has no external link.
The right head and controls are 35 percent larger, with #142B41 navy text on sky blue.
Only the editor-size persistence key is revisioned for the enlarged layout; audio preset IDs are unchanged.

Validation:

- cargo test -p b18n-plugin --lib: 7 passing, including manual opening, cabinet host gesture,
  knob drag gesture and real egui mesh rendering.
- cargo clippy -p b18n-plugin --all-targets -- -D warnings: passed.
- cargo xtask bundle b18n-plugin --release: Windows VST3 bundle.
- Headless renders of the actual editor and manual were visually inspected.

Bundle: target/bundled/B18N 67.vst3.
Preview: target/ui-preview/b18n-editor.png and target/ui-preview/b18n-manual.png.
To regenerate a preview, set B18N_UI_PREVIEW to an output directory and run the library tests.
Set B18N_UI_PREVIEW_MANUAL=1 to render the manual view.

Pending manual validation: native opening/closing in a DAW, Windows DPI behavior,
real host automation playback, preset recovery and listening. The headless images
validate layout and widget interaction, not the DAW's native OpenGL window.
The editor has a fixed initial size; no user-facing resize control is included.

Input routing revision: the selected input displays a watercolor connected plug/cable overlay.
One shared texture is anchored at Input 1 (Bass), Input 2 (Guitar), or INSTR (Instrument),
so all three states retain identical artwork. Knob percentages are always visible.
Thin white brackets identify each channel's knobs; input guides route Input 1/2 into
channel 1 and INSTR into channel 2, with a disconnected crossing. The right lower control
strip starts at y=610 and uses white smoke #F5F5F5. The orange branding column remains intact.
Set B18N_UI_PREVIEW_INPUT=2 or instr for the alternate connected-cable previews.
The input-jack interaction test checks all three host parameter gestures.

Cable overlay correction: female socket hardware has been removed from the cable image.
The selected male plug is drawn onto the single original socket already in the head artwork.
