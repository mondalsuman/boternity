//! Semantic text chunker for file content.
//!
//! Uses the `text-splitter` crate to split text files into semantic chunks
//! suitable for embedding. Markdown files use `MarkdownSplitter` for
//! heading-aware splitting; all other text uses `TextSplitter`.
//!
//! Chunk target size: 512 characters with paragraph boundary awareness.

use text_splitter::{MarkdownSplitter, TextSplitter};

/// Default chunk size in characters.
///
/// 512 characters gives a good balance between semantic coherence and
/// embedding model context window usage for BGESmallENV15.
pub const DEFAULT_CHUNK_SIZE: usize = 512;

/// Result of chunking a text file.
#[derive(Debug)]
pub struct ChunkResult {
    /// The individual text chunks, in order.
    pub chunks: Vec<String>,
    /// Whether markdown-aware splitting was used.
    pub is_markdown: bool,
}

/// Chunk a text file into semantic pieces.
///
/// Uses `MarkdownSplitter` for `.md` files to preserve heading-level boundaries.
/// Uses `TextSplitter` for all other text to split by paragraph/sentence boundaries.
///
/// # Arguments
///
/// * `text` - The full text content to chunk.
/// * `filename` - The filename (used to detect if content is Markdown).
/// * `chunk_size` - Target chunk size in characters. Pass `None` for the default (512).
///
/// # Returns
///
/// A `ChunkResult` with the ordered chunks and whether markdown splitting was used.
pub fn chunk_text_file(text: &str, filename: &str, chunk_size: Option<usize>) -> ChunkResult {
    let size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);

    if text.is_empty() {
        return ChunkResult {
            chunks: vec![],
            is_markdown: false,
        };
    }

    let is_markdown = is_markdown_file(filename);

    let chunks: Vec<String> = if is_markdown {
        let splitter = MarkdownSplitter::new(size);
        splitter.chunks(text).map(String::from).collect()
    } else {
        let splitter = TextSplitter::new(size);
        splitter.chunks(text).map(String::from).collect()
    };

    ChunkResult { chunks, is_markdown }
}

/// Check if a filename indicates Markdown content.
fn is_markdown_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_plain_text() {
        let text = "First paragraph about Rust programming.\n\nSecond paragraph about async await patterns.\n\nThird paragraph about error handling in Rust.";
        let result = chunk_text_file(text, "notes.txt", Some(60));

        assert!(!result.is_markdown);
        assert!(result.chunks.len() >= 2);
        // All chunks should be non-empty
        assert!(result.chunks.iter().all(|c| !c.is_empty()));
    }

    #[test]
    fn test_chunk_markdown() {
        let text = "# Heading 1\n\nSome content under heading 1.\n\n## Heading 2\n\nContent under heading 2 with more details.\n\n## Heading 3\n\nFinal section with conclusions.";
        let result = chunk_text_file(text, "document.md", Some(60));

        assert!(result.is_markdown);
        assert!(result.chunks.len() >= 2);
        assert!(result.chunks.iter().all(|c| !c.is_empty()));
    }

    #[test]
    fn test_chunk_empty_text() {
        let result = chunk_text_file("", "empty.txt", None);
        assert!(result.chunks.is_empty());
        assert!(!result.is_markdown);
    }

    #[test]
    fn test_chunk_short_text() {
        let text = "Short text";
        let result = chunk_text_file(text, "short.txt", None);
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0], "Short text");
    }

    #[test]
    fn test_default_chunk_size() {
        assert_eq!(DEFAULT_CHUNK_SIZE, 512);
    }

    #[test]
    fn test_is_markdown_file() {
        assert!(is_markdown_file("notes.md"));
        assert!(is_markdown_file("README.MD"));
        assert!(is_markdown_file("doc.markdown"));
        assert!(is_markdown_file("doc.MARKDOWN"));
        assert!(!is_markdown_file("notes.txt"));
        assert!(!is_markdown_file("code.rs"));
        assert!(!is_markdown_file("data.json"));
    }

    #[test]
    fn test_chunk_preserves_order() {
        let text = "Section A content.\n\nSection B content.\n\nSection C content.";
        let result = chunk_text_file(text, "test.txt", Some(30));

        // Joined chunks should reconstruct (approximately) the original
        let joined = result.chunks.join(" ");
        assert!(joined.contains("Section A"));
        assert!(joined.contains("Section B"));
        assert!(joined.contains("Section C"));
    }

    #[test]
    fn test_chunk_large_text() {
        // Create a large text with many paragraphs
        let paragraphs: Vec<String> = (0..50)
            .map(|i| {
                format!(
                    "This is paragraph number {}. It contains some meaningful content about topic {} that should be chunked properly.",
                    i, i
                )
            })
            .collect();
        let text = paragraphs.join("\n\n");

        let result = chunk_text_file(&text, "large.txt", Some(512));

        // Should produce multiple chunks
        assert!(result.chunks.len() > 1);
        // No chunk should exceed the target size by too much
        // (text-splitter may slightly exceed for single semantic units)
        for chunk in &result.chunks {
            assert!(
                chunk.len() <= 1024,
                "Chunk too large: {} chars",
                chunk.len()
            );
        }
    }
}
