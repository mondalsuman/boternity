//! Smart defaults for purpose categories.
//!
//! Maps each `PurposeCategory` to sensible model/temperature/token/personality
//! defaults, and provides a keyword-based heuristic classifier to categorize
//! free-text bot descriptions.

use boternity_types::builder::PurposeCategory;

// ---------------------------------------------------------------------------
// SmartDefaults
// ---------------------------------------------------------------------------

/// Adaptive defaults tuned to a purpose category.
///
/// These drive the builder's initial suggestions for model configuration,
/// personality tone, and skill attachment. The user can override any value
/// during the interactive builder flow.
#[derive(Debug, Clone)]
pub struct SmartDefaults {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub suggested_tone: String,
    pub suggested_traits: Vec<String>,
    pub suggested_skills: Vec<String>,
}

/// Return smart defaults tuned for the given purpose category.
///
/// | Category        | Temp | MaxTokens | Tone        |
/// |-----------------|------|-----------|-------------|
/// | SimpleUtility   | 0.3  | 2048      | direct      |
/// | ComplexAnalyst   | 0.5  | 4096      | analytical  |
/// | Creative        | 0.9  | 4096      | expressive  |
/// | Coding          | 0.2  | 4096      | technical   |
/// | Research        | 0.5  | 4096      | scholarly   |
/// | CustomerService | 0.4  | 2048      | empathetic  |
/// | Custom(_)       | 0.7  | 4096      | adaptive    |
pub fn smart_defaults_for_category(category: &PurposeCategory) -> SmartDefaults {
    let model = "claude-sonnet-4-20250514".to_string();

    match category {
        PurposeCategory::SimpleUtility => SmartDefaults {
            model,
            temperature: 0.3,
            max_tokens: 2048,
            suggested_tone: "direct".to_string(),
            suggested_traits: vec!["efficient".to_string(), "concise".to_string()],
            suggested_skills: vec!["web-search".to_string()],
        },
        PurposeCategory::ComplexAnalyst => SmartDefaults {
            model,
            temperature: 0.5,
            max_tokens: 4096,
            suggested_tone: "analytical".to_string(),
            suggested_traits: vec![
                "thorough".to_string(),
                "precise".to_string(),
                "structured".to_string(),
            ],
            suggested_skills: vec!["web-search".to_string(), "data-analysis".to_string()],
        },
        PurposeCategory::Creative => SmartDefaults {
            model,
            temperature: 0.9,
            max_tokens: 4096,
            suggested_tone: "expressive".to_string(),
            suggested_traits: vec![
                "creative".to_string(),
                "imaginative".to_string(),
                "playful".to_string(),
            ],
            suggested_skills: vec!["text-generation".to_string()],
        },
        PurposeCategory::Coding => SmartDefaults {
            model,
            temperature: 0.2,
            max_tokens: 4096,
            suggested_tone: "technical".to_string(),
            suggested_traits: vec![
                "precise".to_string(),
                "methodical".to_string(),
                "pragmatic".to_string(),
            ],
            suggested_skills: vec!["code-review".to_string(), "code-generation".to_string()],
        },
        PurposeCategory::Research => SmartDefaults {
            model,
            temperature: 0.5,
            max_tokens: 4096,
            suggested_tone: "scholarly".to_string(),
            suggested_traits: vec![
                "thorough".to_string(),
                "citation-aware".to_string(),
                "balanced".to_string(),
            ],
            suggested_skills: vec!["web-search".to_string(), "summarize".to_string()],
        },
        PurposeCategory::CustomerService => SmartDefaults {
            model,
            temperature: 0.4,
            max_tokens: 2048,
            suggested_tone: "empathetic".to_string(),
            suggested_traits: vec![
                "patient".to_string(),
                "helpful".to_string(),
                "clear".to_string(),
            ],
            suggested_skills: vec!["knowledge-base".to_string()],
        },
        PurposeCategory::Custom(_) => SmartDefaults {
            model,
            temperature: 0.7,
            max_tokens: 4096,
            suggested_tone: "adaptive".to_string(),
            suggested_traits: vec!["versatile".to_string()],
            suggested_skills: vec![],
        },
    }
}

// ---------------------------------------------------------------------------
// Purpose classification
// ---------------------------------------------------------------------------

/// Keyword lists per category, checked in priority order (first match wins).
const SIMPLE_UTILITY_KEYWORDS: &[&str] =
    &["email", "reminder", "timer", "calculator", "converter"];

const COMPLEX_ANALYST_KEYWORDS: &[&str] =
    &["analyze", "data", "report", "metrics", "dashboard"];

const CREATIVE_KEYWORDS: &[&str] = &["write", "story", "poem", "creative", "art", "music"];

const CODING_KEYWORDS: &[&str] = &["code", "debug", "program", "develop", "software"];

const RESEARCH_KEYWORDS: &[&str] = &["research", "study", "academic", "paper", "literature"];

const CUSTOMER_SERVICE_KEYWORDS: &[&str] = &["support", "customer", "help desk", "ticket"];

/// Classify a free-text bot description into a `PurposeCategory`.
///
/// Uses simple case-insensitive keyword matching with first-match-wins
/// priority. This is the heuristic half of the "hybrid approach" -- the
/// LLM builder agent provides the judgment half during conversation.
///
/// If no keyword matches, returns `Custom` with the first 50 characters
/// of the description.
pub fn classify_purpose(description: &str) -> PurposeCategory {
    let lower = description.to_lowercase();

    if contains_any(&lower, SIMPLE_UTILITY_KEYWORDS) {
        PurposeCategory::SimpleUtility
    } else if contains_any(&lower, COMPLEX_ANALYST_KEYWORDS) {
        PurposeCategory::ComplexAnalyst
    } else if contains_any(&lower, CREATIVE_KEYWORDS) {
        PurposeCategory::Creative
    } else if contains_any(&lower, CODING_KEYWORDS) {
        PurposeCategory::Coding
    } else if contains_any(&lower, RESEARCH_KEYWORDS) {
        PurposeCategory::Research
    } else if contains_any(&lower, CUSTOMER_SERVICE_KEYWORDS) {
        PurposeCategory::CustomerService
    } else {
        let truncated: String = description.chars().take(50).collect();
        PurposeCategory::Custom(truncated)
    }
}

/// Check if `haystack` contains any of the given keywords.
fn contains_any(haystack: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| haystack.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_purpose tests ---

    #[test]
    fn test_classify_simple_utility_keywords() {
        assert_eq!(
            classify_purpose("Build me an email sorter"),
            PurposeCategory::SimpleUtility
        );
        assert_eq!(
            classify_purpose("Daily reminder bot"),
            PurposeCategory::SimpleUtility
        );
        assert_eq!(
            classify_purpose("Unit converter"),
            PurposeCategory::SimpleUtility
        );
    }

    #[test]
    fn test_classify_complex_analyst_keywords() {
        assert_eq!(
            classify_purpose("Analyze my sales data"),
            PurposeCategory::ComplexAnalyst
        );
        assert_eq!(
            classify_purpose("Weekly metrics dashboard"),
            PurposeCategory::ComplexAnalyst
        );
        assert_eq!(
            classify_purpose("Generate quarterly report"),
            PurposeCategory::ComplexAnalyst
        );
    }

    #[test]
    fn test_classify_creative_keywords() {
        assert_eq!(
            classify_purpose("Write short stories"),
            PurposeCategory::Creative
        );
        assert_eq!(
            classify_purpose("Help me with poetry and poem writing"),
            PurposeCategory::Creative
        );
        assert_eq!(
            classify_purpose("A creative assistant for art projects"),
            PurposeCategory::Creative
        );
    }

    #[test]
    fn test_classify_coding_keywords() {
        assert_eq!(
            classify_purpose("Help me debug Rust code"),
            PurposeCategory::Coding
        );
        assert_eq!(
            classify_purpose("Software development assistant"),
            PurposeCategory::Coding
        );
        assert_eq!(
            classify_purpose("A bot that helps program in Python"),
            PurposeCategory::Coding
        );
    }

    #[test]
    fn test_classify_research_keywords() {
        assert_eq!(
            classify_purpose("Research assistant for academic papers"),
            PurposeCategory::Research
        );
        assert_eq!(
            classify_purpose("Help me study for exams"),
            PurposeCategory::Research
        );
        assert_eq!(
            classify_purpose("Literature review helper"),
            PurposeCategory::Research
        );
    }

    #[test]
    fn test_classify_customer_service_keywords() {
        assert_eq!(
            classify_purpose("Customer support chatbot"),
            PurposeCategory::CustomerService
        );
        assert_eq!(
            classify_purpose("Handle support tickets"),
            PurposeCategory::CustomerService
        );
        assert_eq!(
            classify_purpose("Help desk automation"),
            PurposeCategory::CustomerService
        );
    }

    #[test]
    fn test_classify_custom_fallback() {
        let result = classify_purpose("A general-purpose companion");
        match result {
            PurposeCategory::Custom(desc) => {
                assert_eq!(desc, "A general-purpose companion");
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_custom_truncates_at_50_chars() {
        let long_desc =
            "This is a very long description that should be truncated to fifty characters exactly";
        let result = classify_purpose(long_desc);
        match result {
            PurposeCategory::Custom(desc) => {
                assert_eq!(desc.chars().count(), 50);
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_case_insensitive() {
        assert_eq!(
            classify_purpose("Help me CODE in Rust"),
            PurposeCategory::Coding
        );
        assert_eq!(
            classify_purpose("CREATIVE writing"),
            PurposeCategory::Creative
        );
    }

    #[test]
    fn test_classify_first_match_wins() {
        // "write" matches Creative before "code" matches Coding
        assert_eq!(
            classify_purpose("write code documentation"),
            PurposeCategory::Creative
        );
    }

    // --- smart_defaults_for_category tests ---

    #[test]
    fn test_defaults_coding_temperature() {
        let defaults = smart_defaults_for_category(&PurposeCategory::Coding);
        assert!((defaults.temperature - 0.2).abs() < f64::EPSILON);
        assert_eq!(defaults.max_tokens, 4096);
        assert_eq!(defaults.suggested_tone, "technical");
    }

    #[test]
    fn test_defaults_creative_temperature() {
        let defaults = smart_defaults_for_category(&PurposeCategory::Creative);
        assert!((defaults.temperature - 0.9).abs() < f64::EPSILON);
        assert_eq!(defaults.max_tokens, 4096);
        assert_eq!(defaults.suggested_tone, "expressive");
    }

    #[test]
    fn test_defaults_simple_utility() {
        let defaults = smart_defaults_for_category(&PurposeCategory::SimpleUtility);
        assert!((defaults.temperature - 0.3).abs() < f64::EPSILON);
        assert_eq!(defaults.max_tokens, 2048);
        assert_eq!(defaults.suggested_tone, "direct");
        assert_eq!(
            defaults.suggested_traits,
            vec!["efficient", "concise"]
        );
        assert_eq!(defaults.suggested_skills, vec!["web-search"]);
    }

    #[test]
    fn test_defaults_custom_category() {
        let defaults =
            smart_defaults_for_category(&PurposeCategory::Custom("anything".to_string()));
        assert!((defaults.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(defaults.max_tokens, 4096);
        assert_eq!(defaults.suggested_tone, "adaptive");
        assert_eq!(defaults.suggested_traits, vec!["versatile"]);
        assert!(defaults.suggested_skills.is_empty());
    }

    #[test]
    fn test_defaults_all_use_same_model() {
        let categories = [
            PurposeCategory::SimpleUtility,
            PurposeCategory::ComplexAnalyst,
            PurposeCategory::Creative,
            PurposeCategory::Coding,
            PurposeCategory::Research,
            PurposeCategory::CustomerService,
            PurposeCategory::Custom("test".to_string()),
        ];
        for cat in &categories {
            let defaults = smart_defaults_for_category(cat);
            assert_eq!(
                defaults.model, "claude-sonnet-4-20250514",
                "model mismatch for {cat:?}"
            );
        }
    }

    #[test]
    fn test_defaults_customer_service() {
        let defaults = smart_defaults_for_category(&PurposeCategory::CustomerService);
        assert!((defaults.temperature - 0.4).abs() < f64::EPSILON);
        assert_eq!(defaults.max_tokens, 2048);
        assert_eq!(defaults.suggested_tone, "empathetic");
        assert_eq!(
            defaults.suggested_traits,
            vec!["patient", "helpful", "clear"]
        );
    }

    #[test]
    fn test_defaults_research() {
        let defaults = smart_defaults_for_category(&PurposeCategory::Research);
        assert!((defaults.temperature - 0.5).abs() < f64::EPSILON);
        assert_eq!(defaults.suggested_tone, "scholarly");
        assert_eq!(
            defaults.suggested_skills,
            vec!["web-search", "summarize"]
        );
    }

    #[test]
    fn test_defaults_complex_analyst() {
        let defaults = smart_defaults_for_category(&PurposeCategory::ComplexAnalyst);
        assert!((defaults.temperature - 0.5).abs() < f64::EPSILON);
        assert_eq!(defaults.suggested_tone, "analytical");
        assert_eq!(
            defaults.suggested_skills,
            vec!["web-search", "data-analysis"]
        );
    }
}
