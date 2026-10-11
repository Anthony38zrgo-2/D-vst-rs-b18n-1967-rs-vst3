//! Adaptador mínimo del bundler NIH-plug (Hito E, implementado en la Ola 7).
//!
//! `cargo xtask bundle b18n-plugin --release` genera
//! `target/bundled/B18N 67.vst3` (la clave de `bundler.toml` debe coincidir
//! con el nombre de paquete). El commit de NIH-plug está fijado para
//! garantizar compilaciones reproducibles.

fn main() -> nih_plug_xtask::Result<()> {
    nih_plug_xtask::main()
}
