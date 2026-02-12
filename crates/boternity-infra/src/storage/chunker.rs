//! Semantic text chunker for file content.
//!
//! Uses the `text-splitter` crate to split text files into semantic chunks
//! suitable for embedding. Markdown files use `MarkdownSplitter` for
//! heading-aware splitting; all other text uses `TextSplitter`.
//!
//! Chunk target size: 512 characters with paragraph boundary awareness.
