//! Numbers, sizes, dates and durations as the current locale writes them. The words
//! and the order of the parts come from the locale's [`FormatMsg`] table; the digits
//! are grouped and separated by its `GroupSeparator` and `DecimalSeparator`.

use chrono::{Datelike as _, NaiveDate};

use super::{FormatMsg, FormatPlural, Locale, locale, t, tr, trn};

/// A whole number with its digits grouped: "4,091" in English, "4 091" in French.
pub fn number(value: u64) -> String {
    group(&value.to_string(), t(FormatMsg::GroupSeparator))
}

/// A signed whole number with its digits grouped.
pub fn signed(value: i64) -> String {
    if value < 0 {
        format!("-{}", number(value.unsigned_abs()))
    } else {
        number(value as u64)
    }
}

/// A number with `places` decimals and grouped digits: "1.5" or "1,5".
pub fn decimal(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text.as_str(), None),
    };
    let (sign, digits) = match whole.strip_prefix('-') {
        Some(digits) => ("-", digits),
        None => ("", whole),
    };
    let mut out = format!("{sign}{}", group(digits, t(FormatMsg::GroupSeparator)));
    if let Some(fraction) = fraction {
        out.push_str(t(FormatMsg::DecimalSeparator));
        out.push_str(fraction);
    }
    out
}

/// Separates the digits in groups of three, from the right.
fn group(digits: &str, separator: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 * separator.len());
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

/// A percentage of a whole number: "45%", "45 %".
pub fn percent(value: u64) -> String {
    tr!(FormatMsg::Percent, value = number(value))
}

/// A file's or a folder's size in decimal units, as Finder shows sizes: "512 bytes",
/// "4.2 MB", "38 MB", "1.2 GB". One decimal below 10.
pub fn bytes(bytes: u64) -> String {
    sized(bytes, 1000.0)
}

/// A size in binary units (1 KB is 1,024 bytes), as a download's or a memory's size
/// is often counted: "1.5 MB".
pub fn bytes_binary(bytes: u64) -> String {
    sized(bytes, 1024.0)
}

fn sized(bytes: u64, step: f64) -> String {
    const UNITS: [FormatMsg; 4] = [
        FormatMsg::Kilobytes,
        FormatMsg::Megabytes,
        FormatMsg::Gigabytes,
        FormatMsg::Terabytes,
    ];
    if (bytes as f64) < step {
        return trn!(FormatPlural::Bytes, bytes);
    }
    let mut value = bytes as f64 / step;
    let mut unit = 0;
    while value >= step && unit < UNITS.len() - 1 {
        value /= step;
        unit += 1;
    }
    let places = if value < 10.0 { 1 } else { 0 };
    tr!(UNITS[unit], size = decimal(value, places))
}

/// Memory in MiB, as the memory settings show it: "512 MB", "4 GB", "4.5 GB".
pub fn memory_mb(mb: u64) -> String {
    if mb < 1024 {
        tr!(FormatMsg::Megabytes, size = number(mb))
    } else if mb % 1024 == 0 {
        tr!(FormatMsg::Gigabytes, size = number(mb / 1024))
    } else {
        tr!(FormatMsg::Gigabytes, size = decimal(mb as f64 / 1024.0, 1))
    }
}

/// A count in a few characters, as for downloads: "950", "1.2k", "3.4M" in English,
/// "1,2 k" in French, and "1.2万" or "3.4亿" (ten thousands, hundred millions) in
/// Chinese.
pub fn compact(value: u64) -> String {
    let (thousand, million) = match locale() {
        Locale::ZhHans | Locale::ZhHant => (10_000.0, 100_000_000.0),
        Locale::En | Locale::Fr => (1_000.0, 1_000_000.0),
    };
    let value_f = value as f64;
    if value_f < thousand {
        number(value)
    } else if value_f < million {
        tr!(
            FormatMsg::CompactThousands,
            value = decimal(value_f / thousand, 1)
        )
    } else {
        tr!(
            FormatMsg::CompactMillions,
            value = decimal(value_f / million, 1)
        )
    }
}

/// Items in a list, joined as the locale joins them: "a, b, c" or "a、b、c".
pub fn list<S: AsRef<str>>(items: &[S]) -> String {
    let separator = t(FormatMsg::ListSeparator);
    let mut out = String::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(separator);
        }
        out.push_str(item.as_ref());
    }
    out
}

/// A day of the year, short: "Mar 12" ("12 mars", "3月12日"), with the year when
/// `with_year`: "Mar 12, 2025".
pub fn short_date(date: NaiveDate, with_year: bool) -> String {
    let month = t(MONTHS[date.month0() as usize]);
    if with_year {
        tr!(
            FormatMsg::ShortDateWithYear,
            month = month,
            day = date.day(),
            year = date.year()
        )
    } else {
        tr!(FormatMsg::ShortDate, month = month, day = date.day())
    }
}

const MONTHS: [FormatMsg; 12] = [
    FormatMsg::January,
    FormatMsg::February,
    FormatMsg::March,
    FormatMsg::April,
    FormatMsg::May,
    FormatMsg::June,
    FormatMsg::July,
    FormatMsg::August,
    FormatMsg::September,
    FormatMsg::October,
    FormatMsg::November,
    FormatMsg::December,
];

/// How long an instance has been played: "2 h 5 min", "12 min", "Less than a
/// minute", or "Not played yet" for none.
pub fn play_time(seconds: u64) -> String {
    match seconds {
        0 => t(FormatMsg::NotPlayedYet).into(),
        1..60 => t(FormatMsg::LessThanAMinute).into(),
        60..3600 => tr!(FormatMsg::Minutes, minutes = number(seconds / 60)),
        _ => tr!(
            FormatMsg::HoursMinutes,
            hours = number(seconds / 3600),
            minutes = seconds / 60 % 60
        ),
    }
}

/// A time today or yesterday with the day's word, else the date: "Today, 14:30",
/// "Yesterday, 09:12", "2026-10-30".
pub fn day_and_time(time: chrono::DateTime<chrono::Local>, today: NaiveDate) -> String {
    let day = time.date_naive();
    let clock = time.format("%H:%M").to_string();
    if day == today {
        tr!(FormatMsg::TodayAt, time = clock)
    } else if today.pred_opt() == Some(day) {
        tr!(FormatMsg::YesterdayAt, time = clock)
    } else {
        time.format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Msg, interpolate};

    /// Formats with `locale`'s table, without changing the global locale the other
    /// tests read.
    fn in_locale(locale: Locale, message: FormatMsg) -> &'static str {
        Msg::from(message).text(locale)
    }

    #[test]
    fn groups_digits() {
        assert_eq!(group("4091", ","), "4,091");
        assert_eq!(group("409", ","), "409");
        assert_eq!(group("1234567", "\u{202F}"), "1\u{202F}234\u{202F}567");
        assert_eq!(group("", ","), "");
    }

    #[test]
    fn locale_punctuation() {
        assert_eq!(in_locale(Locale::En, FormatMsg::DecimalSeparator), ".");
        assert_eq!(in_locale(Locale::Fr, FormatMsg::DecimalSeparator), ",");
        assert_eq!(
            interpolate(
                in_locale(Locale::Fr, FormatMsg::Gigabytes),
                &[("size", &"1,2")]
            ),
            "1,2\u{a0}Go"
        );
        assert_eq!(
            interpolate(
                in_locale(Locale::ZhHans, FormatMsg::TodayAt),
                &[("time", &"14:30")]
            ),
            "今天 14:30"
        );
    }
}
