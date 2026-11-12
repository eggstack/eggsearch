use crate::core::document::RenderedBlock;
use crate::core::sanitize::bound_text;
use crate::fetch::render;

pub(crate) fn project_document_text(
    blocks: &[RenderedBlock],
    window_chars: usize,
    markdown: bool,
) -> (String, bool) {
    if blocks.is_empty() {
        return (String::new(), false);
    }
    let mut windowed = blocks.to_vec();
    let block_cut = render::blocks::truncate_blocks_at_budget(&mut windowed, window_chars);
    let rendered = if markdown {
        render::markdown::render_blocks_markdown(&windowed)
    } else {
        render::text::render_blocks_text(&windowed)
    };
    let (bounded, text_cut) = bound_text(&rendered, window_chars);
    (bounded, block_cut || text_cut)
}

#[cfg(test)]
mod tests {
    use super::project_document_text;
    use crate::core::document::{BlockKind, RenderedBlock};

    fn block(text: &str, kind: BlockKind) -> RenderedBlock {
        RenderedBlock {
            kind,
            text: text.to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }
    }

    #[test]
    fn a_fitting_document_is_returned_whole() {
        let blocks = vec![
            block("alpha", BlockKind::Paragraph),
            block("beta", BlockKind::Paragraph),
        ];
        let (text, cut) = project_document_text(&blocks, 100, false);
        assert_eq!(text, "alpha\n\nbeta");
        assert!(!cut);
    }

    #[test]
    fn a_window_that_cuts_a_block_truncates_at_the_block_boundary() {
        let blocks = vec![
            block(&"a".repeat(60), BlockKind::Paragraph),
            block(&"b".repeat(60), BlockKind::Paragraph),
        ];
        let (text, cut) = project_document_text(&blocks, 80, false);
        assert_eq!(text, "a".repeat(60));
        assert!(cut);
    }

    #[test]
    fn an_empty_block_list_projects_to_empty_text() {
        let (text, cut) = project_document_text(&[], 10, false);
        assert!(text.is_empty());
        assert!(!cut);
    }

    #[test]
    fn the_projection_does_not_mutate_its_input() {
        let blocks = vec![
            block(&"a".repeat(60), BlockKind::Paragraph),
            block(&"b".repeat(60), BlockKind::Paragraph),
        ];
        let before = blocks.len();
        let _ = project_document_text(&blocks, 10, false);
        assert_eq!(blocks.len(), before);
        assert_eq!(blocks[1].text, "b".repeat(60));
    }
}
