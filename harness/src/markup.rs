//! Harness stub of the one markup entry point model.rs uses. The real
//! links() parses rich markup via egui layout code; this stub recognizes
//! bare http(s) URLs and bare domain.name tokens, normalized like the real
//! previews do. Only preview generation touches it.
pub fn links(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != ':' && c != '-' && c != '_' && c != '~' && c != '%' && c != '?' && c != '&' && c != '=' && c != '#');
        let normalized = if word.starts_with("http://") || word.starts_with("https://") {
            word.to_owned()
        } else if looks_like_domain(word) {
            format!("https://{word}/")
        } else {
            continue;
        };
        if !normalized.starts_with("mailto:") && !found.contains(&normalized) {
            found.push(normalized);
        }
    }
    found
}

/// `engine.rocks` yes, `a.b` no: a dot with letters on both sides and a
/// TLD of at least two letters.
fn looks_like_domain(word: &str) -> bool {
    let host = word.split('/').next().unwrap_or(word);
    let Some((name, tld)) = host.rsplit_once('.') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.')
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_alphabetic())
}
