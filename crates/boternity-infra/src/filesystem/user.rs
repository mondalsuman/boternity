//! USER.md file operations.
//!
//! USER.md is a user-curated briefing document. Unlike SOUL.md it has no
//! frontmatter -- it's plain markdown authored by the user.
//!
//! This module provides read/write helpers and validation.

use std::path::Path;

use boternity_core::service::fs::FileSystem;

/// Read USER.md content from disk.
///
/// Returns `None` if the file doesn't exist.
pub async fn read_user_file<F: FileSystem>(
    fs: &F,
    user_path: &Path,
) -> Result<Option<String>, std::io::Error> {
    if !fs.exists(user_path).await {
        return Ok(None);
    }
    let content = fs.read_file(user_path).await?;
    Ok(Some(content))
}

/// Write USER.md content to disk.
pub async fn write_user_file<F: FileSystem>(
    fs: &F,
    user_path: &Path,
    content: &str,
) -> Result<(), std::io::Error> {
    fs.write_file(user_path, content).await
}

/// Check if USER.md has been customized (not just the default template).
///
/// A "customized" USER.md has at least one non-comment, non-heading line
/// with actual content.
pub fn is_user_customized(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("<!--")
            && !trimmed.starts_with("-->")
            && !trimmed.ends_with("-->")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_template_not_customized() {
        let template = r#"# Luna - User Briefing

<!-- This is your personal briefing document for Luna. -->

## Preferences

<!-- How should Luna respond to you? -->

## Standing Instructions

<!-- Things Luna should always keep in mind. -->
"#;
        assert!(!is_user_customized(template));
    }

    #[test]
    fn test_customized_user_detected() {
        let customized = r#"# Luna - User Briefing

## Preferences

I prefer concise answers with bullet points.

## Standing Instructions

<!-- Things Luna should always keep in mind. -->
"#;
        assert!(is_user_customized(customized));
    }

    #[test]
    fn test_empty_not_customized() {
        assert!(!is_user_customized(""));
    }
}
