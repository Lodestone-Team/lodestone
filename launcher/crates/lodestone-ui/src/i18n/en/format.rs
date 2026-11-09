//! Units, dates and durations, used by [`crate::i18n::format`].

crate::i18n::messages! {
    pub enum FormatMsg {
        /// Between groups of three digits: "4,091".
        GroupSeparator = ",",
        /// Before the decimals: "1.5".
        DecimalSeparator = ".",
        /// Between the items of a list: "Fabric, Quilt".
        ListSeparator = ", ",
        Percent = "{value}%",
        Kilobytes = "{size} KB",
        Megabytes = "{size} MB",
        Gigabytes = "{size} GB",
        Terabytes = "{size} TB",
        /// A count in thousands, for downloads: "1.2k".
        CompactThousands = "{value}k",
        /// A count in millions: "3.4M".
        CompactMillions = "{value}M",
        JustNow = "Just now",
        Today = "Today",
        Yesterday = "Yesterday",
        TodayAt = "Today, {time}",
        YesterdayAt = "Yesterday, {time}",
        PlayedJustNow = "Played just now",
        PlayedYesterday = "Played yesterday",
        /// {ago} is a time such as "2 hours ago".
        PlayedAgo = "Played {ago}",
        /// {date} is a short date, "Mar 12".
        PlayedOn = "Played on {date}",
        NotPlayedYet = "Not played yet",
        LessThanAMinute = "Less than a minute",
        Minutes = "{minutes} min",
        HoursMinutes = "{hours} h {minutes} min",
        /// A short date: {month} is the month's short name, "Mar 12".
        ShortDate = "{month} {day}",
        ShortDateWithYear = "{month} {day}, {year}",
        January = "Jan",
        February = "Feb",
        March = "Mar",
        April = "Apr",
        May = "May",
        June = "Jun",
        July = "Jul",
        August = "Aug",
        September = "Sep",
        October = "Oct",
        November = "Nov",
        December = "Dec",
    }

    pub enum FormatPlural {
        Bytes = ("{count} byte", "{count} bytes"),
        MinutesAgo = ("{count} minute ago", "{count} minutes ago"),
        HoursAgo = ("{count} hour ago", "{count} hours ago"),
        DaysAgo = ("{count} day ago", "{count} days ago"),
    }
}
