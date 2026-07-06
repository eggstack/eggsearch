/// HTML structural block builder: parses HTML and produces `Vec<RenderedBlock>`.
pub mod blocks;
/// Line-preserving renderer for code, config, diff, and plain text.
pub mod code;
/// CSV/TSV renderer: produces bounded table previews.
pub mod csv;
/// Markdown renderer: converts HTML blocks to Markdown text.
pub mod markdown;
/// Markdown source file renderer: parses .md files into structured blocks.
pub mod markdown_source;
/// Jupyter notebook renderer: extracts markdown and code cells.
pub mod notebook;
/// Plain text renderer: converts blocks to plain text.
pub mod text;

pub use blocks::{render_blocks, RenderedBlocks};
pub use code::{render_code, render_diff, render_plaintext, RenderedContent};
pub use csv::render_csv;
pub use markdown::render_blocks_markdown;
pub use markdown_source::{render_markdown_source, RenderedMarkdown};
pub use notebook::render_notebook;
pub use text::render_blocks_text;
