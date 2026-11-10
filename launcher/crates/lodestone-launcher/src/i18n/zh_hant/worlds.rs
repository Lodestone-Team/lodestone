use crate::i18n::{WorldsMsg as M, WorldsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Worlds => "世界",
        M::Screenshots => "截圖",

        M::StopGameFirst => "請先停止遊戲：遊戲正開著這個世界，並會在結束時儲存它。",
        M::WaitForTask => "請等候目前的工作完成。",
        M::Cancelled => "已取消。",
        M::GameRunning => "遊戲正在執行：可以開啟世界、檢視備份，但變更要等遊戲停止後才能進行。",

        M::BackingUp => "正在備份 {name}",
        M::RestoringBackup => "正在還原備份",
        M::ReplacingWorld => "正在以備份取代世界",
        M::MovingToTrash => "正在將 {name} 移到{trash}",
        M::MovingBackupToTrash => "正在將備份移到{trash}",
        M::Renaming => "正在重新命名",
        M::Exporting => "正在匯出 {name}",
        M::ImportingWorlds => "正在匯入世界",

        M::BackedUp => "已備份 {name}（{size}）。",
        M::RestoredAsCopy => "已還原為新世界 {folder}。",
        M::Restored => "已還原 {folder}。",
        M::RestoredWithSafety => "已還原 {folder}。原本的世界已備份為 {backup}，並已移到{trash}。",
        M::InTrash => "{name} 已移到{trash}。",
        M::RenamedTo => "已重新命名為 {name}。",
        M::RenamedToFolder => "已重新命名為 {name}（資料夾 {folder}）。",
        M::Exported => "已將 {name} 匯出至 {path}（{size}）。",
        M::Imported => "已匯入 {worlds}。",
        M::FileError => "{name}：{error}",
        M::ZipsOnly => "世界只能從 .zip 檔案匯入。",
        M::ImportWorldPrompt => "匯入世界",
        M::CopiedScreenshot => "已複製截圖。",
        M::CouldntCopy => "無法複製：{error}",

        M::Hardcore => "極限",
        M::Survival => "生存",
        M::Creative => "創造",
        M::Adventure => "冒險",
        M::Spectator => "旁觀",
        M::Cheats => "允許作弊",
        M::Snapshot => "{version}（快照）",
        M::PlayedAt => "上次遊玩：{time}",
        M::BackUp => "建立備份",
        M::Backups => "備份",
        M::Export => "匯出…",
        M::DeleteWorld => "刪除…",
        M::NoBackupsYet => {
            "還沒有備份。「建立備份」會將世界儲存為 zip 檔，放在實例的 backups 資料夾中。"
        }
        M::RestoreAsCopy => "還原為副本",
        M::ReplaceWorld => "取代世界…",

        M::DataPacks => "資料包",
        M::ReadingDataPacks => "正在讀取資料包",
        M::NoDataPacksYet => {
            "這個世界還沒有資料包。加入一個 .zip 檔或資料夾，或在下方從 Modrinth 尋找。"
        }
        M::AddDataPack => "加入資料包…",
        M::AddDataPackPrompt => "加入資料包",
        M::SearchDataPacks => "在 Modrinth 上搜尋資料包，或貼上 Modrinth 連結",
        M::NewPack => "新",
        M::NewPackHint => "遊戲會在下次載入這個世界時啟用它。",
        M::PacksNotSwitchable => {
            "請先用 Minecraft 1.13 或更新版本開啟這個世界一次，之後就能在這裡啟用和停用它的資料包。"
        }
        M::AddingDataPacks => "正在加入資料包",
        M::SavingPackList => "正在儲存世界的資料包清單",
        M::AddedDataPacks => "已將 {packs} 加入 {name}。",
        M::InstalledDataPack => "已將 {pack} 安裝到 {name}。",
        M::UpdatedDataPack => "已將 {pack} 安裝到 {name}，取代了 {old}。",
        M::PackTurnedOn => "{pack} 已啟用，會在下次開啟世界時載入。",
        M::PackTurnedOff => "{pack} 已停用，下次開啟世界時不再載入。",
        M::NoDataPackVersion => "{name} 沒有適用於 Minecraft {version} 的資料包。",
        M::SearchingModrinth => "正在搜尋 Modrinth",
        M::NoDataPacksFound => "找不到適用於 Minecraft {version} 的資料包。",
        M::LinkNotDataPack => "這個連結不是資料包。",
        M::ModrinthUnavailable => "目前無法連線到 Modrinth；仍然可以從檔案加入資料包。",

        M::ImportWorld => "匯入世界…",
        M::OpenSavesFolder => "開啟 saves 資料夾",
        M::ReadingWorlds => "正在讀取世界",
        M::NoWorldsYet => "還沒有世界",
        M::NoWorldsHint => {
            "你在遊戲中建立的世界會顯示在這裡。你也可以匯入世界 zip 檔，或將其拖放到這裡。"
        }

        M::ReadingScreenshots => "正在讀取截圖",
        M::NoScreenshotsYet => "還沒有截圖",
        M::NoScreenshotsHint => "在遊戲中按 F2 即可截圖，截圖會顯示在這裡。",
        M::Position => "第 {index} 張，共 {count} 張",
        M::CopyImage => "複製圖片",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScreenshotsSummary => ["{count} 張截圖 · 在遊戲中按 F2 即可截圖"; 2],
    }
}
