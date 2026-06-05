//! Query-side stop-word stripping for retrieval.
//!
//! SurrealDB 3.x has no built-in stop-words analyzer filter (only `mapper(path)` with a
//! custom dictionary file), so common words are dropped from the *query* before it reaches
//! BM25. This matters because retrieval uses OR semantics (`@n,OR@`): without it, a question
//! like "what about cryptocurrency" matches on "what"/"about" and slips past the coverage
//! gate instead of being refused. Stripping leaves only content words, so out-of-corpus
//! questions retrieve nothing and the gate refuses (the historically-humble stance).
//!
//! Deliberately a small, common English list — not exhaustive. It only affects the *query*,
//! never the stored text, so it cannot hide a real saying; at worst an over-aggressive entry
//! would drop a content word, so the list is kept conservative.

/// Common English function words with no retrieval value.
const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
    "as", "at", "be", "because", "been", "before", "being", "below", "between", "both", "but",
    "by", "can", "did", "do", "does", "doing", "down", "during", "each", "few", "for", "from",
    "further", "had", "has", "have", "having", "he", "her", "here", "hers", "him", "his", "how",
    "i", "if", "in", "into", "is", "it", "its", "just", "me", "more", "most", "my", "no", "nor",
    "not", "of", "off", "on", "once", "only", "or", "other", "our", "out", "over", "own", "same",
    "she", "should", "so", "some", "such", "than", "that", "the", "their", "them", "then", "there",
    "these", "they", "this", "those", "through", "to", "too", "under", "until", "up", "very",
    "was", "we", "were", "what", "when", "where", "which", "while", "who", "whom", "why", "will",
    "with", "would", "you", "your",
];

/// Drop stop words from `query`, returning the remaining content words joined by spaces.
///
/// Tokenizes on non-alphanumerics and lowercases for the stop-word check (matching the
/// analyzer's `lowercase` filter). Returns an empty string when every token is a stop word —
/// the caller treats that as "no coverage".
pub fn strip(query: &str) -> String {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|tok| !tok.is_empty())
        .filter(|tok| !STOP_WORDS.contains(&tok.to_lowercase().as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_content_words() {
        assert_eq!(strip("render to Caesar"), "render Caesar");
    }

    #[test]
    fn all_stopwords_yields_empty() {
        assert_eq!(strip("what is it about"), "");
        assert_eq!(strip("what about"), "");
    }

    #[test]
    fn keeps_the_real_content_word() {
        assert_eq!(strip("what about cryptocurrency"), "cryptocurrency");
    }

    #[test]
    fn empty_query_is_empty() {
        assert_eq!(strip(""), "");
        assert_eq!(strip("   ,. "), "");
    }
}
