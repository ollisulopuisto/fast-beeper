//! signal.art sticker-pack link detection.
//!
//! Importing packs from these links went with the removed native backend.

/// Parses a pack id and key from a signal.art URL or parameter string.
pub fn parse_signal_url(url: &str) -> Result<(String, [u8; 32]), String> {
    let tail = url.rsplit('#').next().unwrap_or(url);
    let mut id = None;
    let mut key = None;
    for part in tail.split(['&', '?', ';']) {
        if let Some(value) = part.strip_prefix("pack_id=") {
            id = Some(value.trim().to_lowercase());
        }
        if let Some(value) = part.strip_prefix("pack_key=") {
            key = Some(value.trim().to_lowercase());
        }
    }
    let id = id
        .filter(|id| id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("Missing pack_id. Copy the full signal.art link")?;
    let key = key
        .and_then(|key| hex_bytes(&key))
        .ok_or("Missing or invalid pack_key. Copy the full signal.art link")?;
    Ok((id, key))
}

fn hex_bytes(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

/// Whether pasted text contains a complete signal.art pack link.
pub fn looks_like_signal_url(text: &str) -> bool {
    parse_signal_url(text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signal_link_gives_up_its_id_and_key() {
        let url = format!(
            "https://signal.art/addstickers/#pack_id={}&pack_key={}",
            "0123456789abcdef0123456789abcdef",
            "aa".repeat(32)
        );
        let (id, key) = parse_signal_url(&url).expect("parses");
        assert_eq!(id, "0123456789abcdef0123456789abcdef");
        assert_eq!(key, [0xaa; 32]);
        assert!(looks_like_signal_url(&url));
        assert!(!looks_like_signal_url("https://signal.art/addstickers/"));
        assert!(!looks_like_signal_url("pack_id=zz&pack_key=short"));
    }
}
