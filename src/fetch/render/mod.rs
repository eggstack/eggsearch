/// HTML structural block builder: parses HTML and produces `Vec<RenderedBlock>`.
pub mod blocks;
/// Markdown renderer: converts blocks to Markdown text.
pub mod markdown;
/// Plain text renderer: converts blocks to plain text.
pub mod text;

pub use blocks::{render_blocks, RenderedBlocks};
pub use markdown::render_blocks_markdown;
pub use text::render_blocks_text;
