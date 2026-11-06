//! Numeric day-separator dates, using the OS locale's date field order.

use super::{Date, Locale};

/// Cache the OS pattern, not a formatted date: separators change at midnight
/// and when the year changes, without restarting the application.
pub(super) fn pattern(region: Option<super::Region>, locale: Locale) -> &'static str {
    static PATTERN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    PATTERN
        .get_or_init(native_pattern)
        .as_deref()
        .unwrap_or_else(|| match region {
            Some(super::Region::Finnish) => "dd.MM.yyyy",
            Some(super::Region::EnglishUs) => "M/d/yyyy",
            None => fallback_pattern(locale),
        })
}

pub(super) fn fallback_pattern(locale: Locale) -> &'static str {
    match locale {
        Locale::English => "MM/dd/yyyy",
        Locale::German | Locale::Russian | Locale::Turkish => "dd.MM.yyyy",
        Locale::ChineseSimplified | Locale::ChineseTraditional => "yyyy/MM/dd",
        _ => "dd/MM/yyyy",
    }
}

#[cfg(target_os = "macos")]
fn native_pattern() -> Option<String> {
    use objc2_foundation::{NSDateFormatter, NSLocale, NSString};
    // The skeleton asks Foundation for numeric month/day/year in the user's
    // region, rather than assuming that the UI language determines its order.
    NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("yyyyMMdd"),
        0,
        Some(&NSLocale::currentLocale()),
    )
    .map(|pattern| pattern.to_string())
}

#[cfg(windows)]
fn native_pattern() -> Option<String> {
    use windows_sys::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SSHORTDATE};
    let mut buffer = [0u16; 128];
    // SAFETY: null selects the user's locale; length is in UTF-16 units.
    let written = unsafe {
        GetLocaleInfoEx(
            std::ptr::null(),
            LOCALE_SSHORTDATE,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    (written > 0).then(|| String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

#[cfg(target_os = "linux")]
fn native_pattern() -> Option<String> {
    // Read LC_TIME without changing the process-global C locale.
    // SAFETY: copy the locale-owned string before freeing its locale.
    unsafe {
        let locale = libc::newlocale(libc::LC_TIME_MASK, c"".as_ptr(), std::ptr::null_mut());
        if locale.is_null() {
            return None;
        }
        let pointer = libc::nl_langinfo_l(libc::D_FMT, locale);
        let value = (!pointer.is_null()).then(|| {
            std::ffi::CStr::from_ptr(pointer)
                .to_string_lossy()
                .into_owned()
        });
        libc::freelocale(locale);
        value.and_then(|value| posix_pattern(&value))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn native_pattern() -> Option<String> {
    None
}

/// Convert libc's numeric date fields to the same pattern syntax used by
/// Foundation and Windows. Unknown directives fall back instead of leaking
/// percent codes or displaying a partially formatted date.
#[cfg(any(target_os = "linux", test))]
fn posix_pattern(pattern: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            // Numeric libc patterns contain punctuation and localized suffixes,
            // not unescaped date-field letters. Reject unexpected field-like
            // text rather than accidentally interpreting it as another field.
            if matches!(c, 'd' | 'M' | 'y' | '\'') {
                return None;
            }
            out.push(c);
            continue;
        }
        let mut field = chars.next()?;
        if matches!(field, '-' | '_' | '0' | 'O' | 'E') {
            field = chars.next()?;
        }
        out.push_str(match field {
            'd' | 'e' => "dd",
            'm' => "MM",
            'y' | 'Y' => "yyyy",
            'F' => "yyyy-MM-dd",
            'D' => "MM/dd/yyyy",
            _ => return None,
        });
    }
    Some(out)
}

#[derive(Debug)]
enum Part {
    Field(char, usize),
    Literal(String),
}

/// Only calendar fields are interpreted; punctuation and quoted literals are
/// preserved. Two-digit year preferences become full years for older dates.
pub(super) fn format(date: Date, with_year: bool, pattern: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut chars = pattern.chars().peekable();
    let mut quoted = false;
    let mut literal = String::new();
    while let Some(c) = chars.next() {
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                literal.push('\'');
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if !quoted && matches!(c, 'd' | 'M' | 'y') {
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(&mut literal)));
            }
            let mut width = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                width += 1;
            }
            // Weekday/month names aren't numeric short dates.
            if c != 'y' && width > 2 {
                return None;
            }
            parts.push(Part::Field(c, width));
        } else {
            literal.push(c);
        }
    }
    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    if quoted
        || !['d', 'M', 'y'].iter().all(|wanted| {
            parts
                .iter()
                .any(|part| matches!(part, Part::Field(field, _) if field == wanted))
        })
    {
        return None;
    }
    if !with_year {
        let year = parts
            .iter()
            .position(|part| matches!(part, Part::Field('y', _)))?;
        parts.remove(year);
        // A leading year owns its following separator/suffix (yyyy年MM月dd日).
        // A trailing year owns its preceding separator, but Finnish's final
        // day/month period remains: dd.MM.yyyy -> dd.MM.
        if year == 0 {
            if matches!(parts.first(), Some(Part::Literal(_))) {
                parts.remove(0);
            }
        } else if let Part::Literal(separator) = &mut parts[year - 1]
            && separator != "."
        {
            separator.clear();
        }
    }
    let mut out = String::new();
    for part in parts {
        match part {
            Part::Field('y', _) => out.push_str(&format!("{:04}", date.year())),
            Part::Field(field, width) => {
                let value = if field == 'd' {
                    date.day()
                } else {
                    date.month()
                };
                if width == 2 {
                    out.push_str(&format!("{value:02}"));
                } else {
                    out.push_str(&value.to_string());
                }
            }
            Part::Literal(text) => out.push_str(&text),
        }
    }
    Some(out.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_patterns_keep_locale_order_and_full_years() {
        let date = jiff::civil::date(2025, 3, 5);
        for (pattern, short, full) in [
            ("dd.MM.yyyy", "05.03.", "05.03.2025"),
            ("M/d/yy", "3/5", "3/5/2025"),
            ("dd/MM/yyyy", "05/03", "05/03/2025"),
            ("yyyy/MM/dd", "03/05", "2025/03/05"),
            ("yyyy'年'MM'月'dd'日'", "03月05日", "2025年03月05日"),
            ("yyyy-MM-dd", "03-05", "2025-03-05"),
        ] {
            assert_eq!(format(date, false, pattern).as_deref(), Some(short));
            assert_eq!(format(date, true, pattern).as_deref(), Some(full));
        }
    }

    #[test]
    fn libc_patterns_and_fallbacks_are_numeric_in_every_ui_locale() {
        assert_eq!(posix_pattern("%d.%m.%Y").as_deref(), Some("dd.MM.yyyy"));
        assert!(posix_pattern("%a %b %d").is_none());
        let korean = posix_pattern("%Y년 %m월 %d일").unwrap();
        assert_eq!(
            format(jiff::civil::date(2025, 3, 5), false, &korean).as_deref(),
            Some("03월 05일")
        );
        let date = jiff::civil::date(2025, 3, 5);
        for locale in Locale::ALL {
            assert!(format(date, false, fallback_pattern(locale)).is_some());
            assert!(
                format(date, true, fallback_pattern(locale))
                    .unwrap()
                    .contains("2025")
            );
        }
    }
}
