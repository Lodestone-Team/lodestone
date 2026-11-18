//! Numbers, sizes, dates and durations as the current locale writes them. The words
//! and the order of the parts come from the locale's [`FormatMsg`] table; the digits
//! are grouped and separated by its `GroupSeparator` and `DecimalSeparator`.

use std::fmt;

use chrono::{Datelike as _, NaiveDate};

use super::{FormatMsg, FormatPlural, Locale, Msg, Plural, locale, t, tr};

/// A whole number with its digits grouped: "4,091" in English, "4 091" in French.
pub fn number(value: u64) -> String {
    number_in(value, locale())
}

fn number_in(value: u64, locale: Locale) -> String {
    group(
        &value.to_string(),
        in_locale(FormatMsg::GroupSeparator, locale),
    )
}

/// A format message in `locale`.
fn in_locale(message: FormatMsg, locale: Locale) -> &'static str {
    Msg::from(message).text(locale)
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
    decimal_in(value, places, locale())
}

fn decimal_in(value: f64, places: usize, locale: Locale) -> String {
    let text = format!("{value:.places$}");
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text.as_str(), None),
    };
    let (sign, digits) = match whole.strip_prefix('-') {
        Some(digits) => ("-", digits),
        None => ("", whole),
    };
    let mut out = format!(
        "{sign}{}",
        group(digits, in_locale(FormatMsg::GroupSeparator, locale))
    );
    if let Some(fraction) = fraction {
        out.push_str(in_locale(FormatMsg::DecimalSeparator, locale));
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
    bytes_parts(bytes).to_string()
}

/// A size in binary units (1 KB is 1,024 bytes), as a download's or a memory's size
/// is often counted: "1.5 MB".
pub fn bytes_binary(bytes: u64) -> String {
    sized_in(bytes, BINARY, unit_of(bytes, BINARY), locale()).to_string()
}

/// A number and its unit, apart, so the unit can be drawn one ink lighter than the
/// number: "4.2" and " MB", or "4,2" and "\u{a0}Mo". The parts come from the
/// locale's template, so a unit written before the number keeps its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    pub value: String,
    /// The template's text before the number (usually none).
    pub before: &'static str,
    /// The template's text after the number: the unit, with its space.
    pub after: &'static str,
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.before, self.value, self.after)
    }
}

/// [`bytes`] as a number and a unit.
pub fn bytes_parts(bytes: u64) -> Measure {
    bytes_parts_in(bytes, locale())
}

fn bytes_parts_in(bytes: u64, locale: Locale) -> Measure {
    sized_in(bytes, DECIMAL, unit_of(bytes, DECIMAL), locale)
}

/// How far a transfer has come, with both numbers in the total's unit so the unit is
/// written once, after the total: "1.2" of "3.4 MB". Returns the done amount's
/// number and the total.
pub fn bytes_fraction(done: u64, total: u64) -> (String, Measure) {
    bytes_fraction_in(done, total, locale())
}

fn bytes_fraction_in(done: u64, total: u64, locale: Locale) -> (String, Measure) {
    let unit = unit_of(total, DECIMAL);
    let done = done.min(total);
    // With the total's decimals, so the two line up: "0.4 / 3.4 MB", not "0 / 3.4 MB".
    let done_value = match unit {
        Some(index) => {
            let scale = DECIMAL.powi(index as i32 + 1);
            decimal_in(done as f64 / scale, places_of(total, unit), locale)
        }
        None => number_in(done, locale),
    };
    (done_value, sized_in(total, DECIMAL, unit, locale))
}

const DECIMAL: f64 = 1000.0;
const BINARY: f64 = 1024.0;

const UNITS: [FormatMsg; 4] = [
    FormatMsg::Kilobytes,
    FormatMsg::Megabytes,
    FormatMsg::Gigabytes,
    FormatMsg::Terabytes,
];

/// The unit `bytes` is shown in: `None` for bytes, else an index into [`UNITS`].
fn unit_of(bytes: u64, step: f64) -> Option<usize> {
    let mut value = bytes as f64;
    if value < step {
        return None;
    }
    let mut unit = 0;
    value /= step;
    while value >= step && unit < UNITS.len() - 1 {
        value /= step;
        unit += 1;
    }
    Some(unit)
}

/// The decimals `bytes` is shown with in `unit`: one below 10, else none.
fn places_of(bytes: u64, unit: Option<usize>) -> usize {
    match unit {
        None => 0,
        Some(index) => {
            let value = bytes as f64 / DECIMAL.powi(index as i32 + 1);
            if value < 10.0 { 1 } else { 0 }
        }
    }
}

/// `bytes` in `unit` (`None` for bytes), as a number and the unit's text.
fn sized_in(bytes: u64, step: f64, unit: Option<usize>, locale: Locale) -> Measure {
    let Some(index) = unit else {
        let [one, other] = Plural::from(FormatPlural::Bytes).forms(locale);
        let template = if locale.is_one(bytes) { one } else { other };
        let (before, after) = split(template, "count");
        return Measure {
            value: number_in(bytes, locale),
            before,
            after,
        };
    };
    let value = bytes as f64 / step.powi(index as i32 + 1);
    let places = if value < 10.0 { 1 } else { 0 };
    let (before, after) = split(in_locale(UNITS[index], locale), "size");
    Measure {
        value: decimal_in(value, places, locale),
        before,
        after,
    }
}

/// The text of `template` before and after its `{name}` placeholder.
fn split(template: &'static str, name: &str) -> (&'static str, &'static str) {
    let placeholder = format!("{{{name}}}");
    match template.find(&placeholder) {
        Some(at) => (&template[..at], &template[at + placeholder.len()..]),
        None => ("", template),
    }
}

/// Memory in MiB, as the memory settings show it: "512 MB", "4 GB", "4.5 GB".
pub fn memory_mb(mb: u64) -> String {
    if mb < 1024 {
        tr!(FormatMsg::Megabytes, size = number(mb))
    } else if mb.is_multiple_of(1024) {
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
    fn sizes_split_into_a_number_and_its_unit() {
        let measure = bytes_parts_in(4_200_000, Locale::En);
        assert_eq!(measure.value, "4.2");
        assert_eq!((measure.before, measure.after), ("", " MB"));
        assert_eq!(measure.to_string(), "4.2 MB");
        let measure = bytes_parts_in(1_234_567_890, Locale::Fr);
        assert_eq!(measure.value, "1,2");
        assert_eq!(measure.after, "\u{a0}Go");
        let measure = bytes_parts_in(38_000_000, Locale::ZhHans);
        assert_eq!(measure.to_string(), "38 MB");
        // bytes use the plural message, in the locale's form
        assert_eq!(bytes_parts_in(1, Locale::En).to_string(), "1 byte");
        assert_eq!(bytes_parts_in(512, Locale::En).to_string(), "512 bytes");
        assert_eq!(bytes_parts_in(0, Locale::Fr).to_string(), "0\u{a0}octet");
        assert_eq!(bytes_parts_in(999, Locale::Fr).after, "\u{a0}octets");
    }

    #[test]
    fn a_fraction_uses_the_total_unit() {
        let (done, total) = bytes_fraction_in(1_200_000, 3_400_000, Locale::En);
        assert_eq!(done, "1.2");
        assert_eq!(total.to_string(), "3.4 MB");
        // done is counted in the total's unit, even when it is far smaller
        let (done, total) = bytes_fraction_in(400_000, 2_500_000_000, Locale::En);
        assert_eq!(done, "0.0");
        assert_eq!(total.to_string(), "2.5 GB");
        let (done, total) = bytes_fraction_in(12, 900, Locale::Fr);
        assert_eq!(done, "12");
        assert_eq!(total.to_string(), "900\u{a0}octets");
        let (done, total) = bytes_fraction_in(25_000_000, 38_000_000, Locale::Fr);
        assert_eq!(done, "25");
        assert_eq!(total.to_string(), "38\u{a0}Mo");
        // more done than the total is shown as the total
        assert_eq!(bytes_fraction_in(5_000, 3_000, Locale::En).0, "3.0");
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
