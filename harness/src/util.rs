//! Harness stub: the two helpers model.rs uses, bodies verbatim from
//! zapfast src/util.rs. The real module also renders SVG icons via egui.
use crate::i18n::Locale;

pub fn duration(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn call_length(locale: Locale, seconds: u64) -> String {
    use crate::i18n::gettext;
    let (hours, minutes, seconds) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    let text = match (hours, minutes, seconds) {
        (0, 0, _) => gettext(locale, "{seconds} s"),
        (0, _, 0) => gettext(locale, "{minutes} min"),
        (0, _, _) => gettext(locale, "{minutes} min {seconds} s"),
        (_, 0, _) => gettext(locale, "{hours} h"),
        _ => gettext(locale, "{hours} h {minutes} min"),
    };
    text.replace("{hours}", &hours.to_string())
        .replace("{minutes}", &minutes.to_string())
        .replace("{seconds}", &seconds.to_string())
}
// ---- real bodies copied from zapfast src/util.rs (verbatim) ----
pub fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}
pub fn phone(digits: &str) -> String {
    let digits: String = digits.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return String::new();
    }
    if let Some(formatted) = brazilian_phone(&digits).or_else(|| french_phone(&digits)) {
        return formatted;
    }
    let (country, national) = digits.split_at(country_code_len(&digits).min(digits.len()));
    let mut out = format!("+{country}");
    // Approximate national grouping: threes, with the last four together
    // rather than leaving a lone digit.
    let mut start = 0;
    while start < national.len() {
        let end = match national.len() - start {
            4 => national.len(),
            _ => (start + 3).min(national.len()),
        };
        out.push(' ');
        out.push_str(&national[start..end]);
        start = end;
    }
    out
}
fn country_code_len(digits: &str) -> usize {
    const TWO_DIGITS: [&str; 44] = [
        "20", "27", "30", "31", "32", "33", "34", "36", "39", "40", "41", "43", "44", "45", "46",
        "47", "48", "49", "51", "52", "53", "54", "55", "56", "57", "58", "60", "61", "62", "63",
        "64", "65", "66", "81", "82", "84", "86", "90", "91", "92", "93", "94", "95", "98",
    ];
    if digits.starts_with(['1', '7']) {
        1
    } else if TWO_DIGITS.iter().any(|code| digits.starts_with(code)) {
        2
    } else {
        3
    }
}
fn brazilian_phone(digits: &str) -> Option<String> {
    let national = digits.strip_prefix("55")?;
    let subscriber_len = match national.len() {
        10 | 11 => national.len() - 2,
        _ => return None,
    };
    let area = &national[..2];
    let subscriber = &national[2..2 + subscriber_len];
    let split = subscriber.len() - 4;
    Some(format!(
        "+55 ({area}) {}-{}",
        &subscriber[..split],
        &subscriber[split..]
    ))
}
fn french_phone(digits: &str) -> Option<String> {
    let (country, national) = ["33", "262", "590", "594", "596"]
        .into_iter()
        .find_map(|code| Some((code, digits.strip_prefix(code)?)))?;
    if national.len() != 9 {
        return None;
    }
    let lead = if country == "33" && !national.starts_with('8') {
        1
    } else {
        3
    };
    let mut out = format!("+{country} {}", &national[..lead]);
    for start in (lead..national.len()).step_by(2) {
        out.push(' ');
        out.push_str(&national[start..start + 2]);
    }
    Some(out)
}
