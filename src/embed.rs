//! Embed static files (console UI) into the binary at compile time.

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct EmbeddedAssets;
