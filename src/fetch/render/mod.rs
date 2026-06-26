/// HTML structural block builder: parses HTML and produces `Vec<RenderedBlock>`.
pub mod blocks;
/// Line-preserving renderer for code, config, diff, and plain text.
pub mod code;
/// Markdown renderer: converts HTML blocks to Markdown text.
pub mod markdown;
/// Markdown source file renderer: parses .md files into structured blocks.
pub mod markdown_source;
/// Plain text renderer: converts blocks to plain text.
pub mod text;

pub use blocks::{render_blocks, RenderedBlocks};
pub use code::{render_code, render_diff, render_plaintext, RenderedContent};
pub use markdown::render_blocks_markdown;
pub use markdown_source::{render_markdown_source, RenderedMarkdown};
pub use text::render_blocks_text;
