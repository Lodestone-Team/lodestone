use crate::i18n::{FormatMsg as M, FormatPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::GroupSeparator => ",",
        M::DecimalSeparator => ".",
        M::ListSeparator => "、",
        M::Percent => "{value}%",
        M::Kilobytes => "{size} KB",
        M::Megabytes => "{size} MB",
        M::Gigabytes => "{size} GB",
        M::Terabytes => "{size} TB",
        // Chinese counts in ten thousands (萬) and hundred millions (億).
        M::CompactThousands => "{value}萬",
        M::CompactMillions => "{value}億",
        M::JustNow => "剛剛",
        M::Today => "今天",
        M::Yesterday => "昨天",
        M::TodayAt => "今天 {time}",
        M::YesterdayAt => "昨天 {time}",
        M::PlayedJustNow => "剛剛玩過",
        M::PlayedYesterday => "昨天玩過",
        M::PlayedAgo => "{ago}玩過",
        M::PlayedOn => "{date}玩過",
        M::NotPlayedYet => "尚未玩過",
        M::LessThanAMinute => "不到 1 分鐘",
        M::Minutes => "{minutes} 分鐘",
        M::HoursMinutes => "{hours} 小時 {minutes} 分鐘",
        M::ShortDate => "{month}{day}日",
        M::ShortDateWithYear => "{year}年{month}{day}日",
        M::January => "1月",
        M::February => "2月",
        M::March => "3月",
        M::April => "4月",
        M::May => "5月",
        M::June => "6月",
        M::July => "7月",
        M::August => "8月",
        M::September => "9月",
        M::October => "10月",
        M::November => "11月",
        M::December => "12月",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Bytes => ["{count} 位元組"; 2],
        P::MinutesAgo => ["{count} 分鐘前"; 2],
        P::HoursAgo => ["{count} 小時前"; 2],
        P::DaysAgo => ["{count} 天前"; 2],
    }
}
