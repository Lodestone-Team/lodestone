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
        // Chinese counts in ten thousands (万) and hundred millions (亿).
        M::CompactThousands => "{value}万",
        M::CompactMillions => "{value}亿",
        M::JustNow => "刚刚",
        M::Today => "今天",
        M::Yesterday => "昨天",
        M::TodayAt => "今天 {time}",
        M::YesterdayAt => "昨天 {time}",
        M::PlayedJustNow => "刚刚玩过",
        M::PlayedYesterday => "昨天玩过",
        M::PlayedAgo => "{ago}玩过",
        M::PlayedOn => "{date}玩过",
        M::NotPlayedYet => "尚未玩过",
        M::LessThanAMinute => "不到 1 分钟",
        M::Minutes => "{minutes} 分钟",
        M::HoursMinutes => "{hours} 小时 {minutes} 分钟",
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
        P::Bytes => ["{count} 字节"; 2],
        P::MinutesAgo => ["{count} 分钟前"; 2],
        P::HoursAgo => ["{count} 小时前"; 2],
        P::DaysAgo => ["{count} 天前"; 2],
    }
}
