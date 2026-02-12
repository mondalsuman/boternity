//! Bot file storage infrastructure.
//!
//! Implements the `FileStore` trait from `boternity-core` for local filesystem
//! storage with version history, plus text chunking and semantic indexing.

use std::path::Path;

pub mod chunker;
pub mod filesystem;
pub mod indexer;

/// Detect MIME type from file extension.
///
/// Used by both `LocalFileStore` (filesystem.rs) and `FileIndexer` (indexer.rs).
pub fn detect_mime(filename: &str) -> String {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // Text
        "txt" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" => "text/xml",
        "yaml" | "yml" => "text/yaml",
        "toml" => "text/toml",

        // Code
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        "json" => "application/json",
        "sh" | "bash" => "text/x-shellscript",
        "sql" => "text/x-sql",
        "go" => "text/x-go",
        "java" => "text/x-java",
        "c" | "h" => "text/x-c",
        "cpp" | "hpp" | "cc" | "cxx" => "text/x-c++",

        // Documents
        "pdf" => "application/pdf",
        "doc" | "docx" => "application/msword",

        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",

        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",

        // Default
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Check whether a MIME type represents indexable text content.
pub fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/") || mime == "application/json"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_mime() {
        assert_eq!(detect_mime("file.txt"), "text/plain");
        assert_eq!(detect_mime("doc.md"), "text/markdown");
        assert_eq!(detect_mime("data.json"), "application/json");
        assert_eq!(detect_mime("image.png"), "image/png");
        assert_eq!(detect_mime("code.rs"), "text/x-rust");
        assert_eq!(detect_mime("unknown.xyz"), "application/octet-stream");
        assert_eq!(detect_mime("no_extension"), "application/octet-stream");
    }

    #[test]
    fn test_is_text_mime() {
        assert!(is_text_mime("text/plain"));
        assert!(is_text_mime("text/markdown"));
        assert!(is_text_mime("text/x-rust"));
        assert!(is_text_mime("application/json"));
        assert!(!is_text_mime("image/png"));
        assert!(!is_text_mime("application/pdf"));
        assert!(!is_text_mime("application/octet-stream"));
    }
}
