use crate::i18n::{FormatMsg as M, FormatPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        // A narrow no-break space, as French groups digits: "4 091".
        M::GroupSeparator => "\u{202F}",
        M::DecimalSeparator => ",",
        M::ListSeparator => ", ",
        M::Percent => "{value}\u{202F}%",
        M::Kilobytes => "{size}\u{a0}ko",
        M::Megabytes => "{size}\u{a0}Mo",
        M::Gigabytes => "{size}\u{a0}Go",
        M::Terabytes => "{size}\u{a0}To",
        M::CompactThousands => "{value}\u{a0}k",
        M::CompactMillions => "{value}\u{a0}M",
        M::JustNow => "À l'instant",
        M::Today => "Aujourd'hui",
        M::Yesterday => "Hier",
        M::TodayAt => "Aujourd'hui, {time}",
        M::YesterdayAt => "Hier, {time}",
        M::PlayedJustNow => "Joué à l'instant",
        M::PlayedYesterday => "Joué hier",
        M::PlayedAgo => "Joué {ago}",
        M::PlayedOn => "Joué le {date}",
        M::NotPlayedYet => "Pas encore joué",
        M::LessThanAMinute => "Moins d'une minute",
        M::Minutes => "{minutes}\u{a0}min",
        M::HoursMinutes => "{hours}\u{a0}h {minutes}\u{a0}min",
        M::ShortDate => "{day} {month}",
        M::ShortDateWithYear => "{day} {month} {year}",
        M::January => "janv.",
        M::February => "févr.",
        M::March => "mars",
        M::April => "avr.",
        M::May => "mai",
        M::June => "juin",
        M::July => "juil.",
        M::August => "août",
        M::September => "sept.",
        M::October => "oct.",
        M::November => "nov.",
        M::December => "déc.",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Bytes => ["{count}\u{a0}octet", "{count}\u{a0}octets"],
        P::MinutesAgo => ["il y a {count}\u{a0}minute", "il y a {count}\u{a0}minutes"],
        P::HoursAgo => ["il y a {count}\u{a0}heure", "il y a {count}\u{a0}heures"],
        P::DaysAgo => ["il y a {count}\u{a0}jour", "il y a {count}\u{a0}jours"],
    }
}
