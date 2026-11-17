use crate::i18n::{ValueMsg as M, ValuePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KeyValue => "{key}：{value}",
        M::FromTo => "從 {from} 變為 {to}",
        M::NeededOfFree => "需要 {needed}，可用 {free}",
        M::Online => "線上",
        M::Offline => "離線",
        M::Running => "執行中",
        M::Stopped => "已停止",
        M::Starting => "正在啟動",
        M::Stopping => "正在停止",
        M::Failed => "失敗",
        M::Installing => "正在安裝",
        M::Downloading => "正在下載",
        M::Copying => "正在複製",
        M::Paused => "已暫停",
        M::KindFolder => "資料夾",
        M::KindWorld => "世界",
        M::KindMod => "模組",
        M::KindResourcePack => "資源包",
        M::KindShaderPack => "光影包",
        M::KindDataPack => "資料包",
        M::KindPlugin => "插件",
        M::KindModpack => "模組包",
        M::KindImage => "圖片",
        M::KindServer => "伺服器",
        M::KindInstance => "實例",
        M::KindArchive => "壓縮檔",
        M::KindFile => "檔案",
        M::KindBackup => "備份",
        M::KindSnapshot => "快照",
        M::KindFriend => "好友",
        M::KindComputer => "電腦",
        M::UntitledSnapshot => "未命名快照",
        M::UntitledBackup => "未命名備份",
        M::BeforeUpdate => "更新前",
        M::BeforeRestore => "還原前",
        M::CopyOf => "複製自",
        M::Automatic => "自動",
        M::Added => "已新增",
        M::Skipped => "已略過",
        M::NotAdded => "未新增",
        M::Reveal => "顯示",
        M::Hide => "隱藏",
        M::Hidden => "已隱藏",
        M::OpenLink => "開啟連結",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShowMore => ["再顯示 {count} 項", "再顯示 {count} 項"],
    }
}
