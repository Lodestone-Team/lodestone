use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "每個備份都是伺服器的完整副本，但未變更的檔案只儲存一次，所以經常備份也不佔多少空間。"
        }
        M::BackUpNow => "立即備份",
        M::BackingUp => "正在備份…",
        M::Restoring => "正在還原…",
        M::JobRunning => "此伺服器正在進行備份或還原。",
        M::NoSettingsPermission => "你需要變更此伺服器設定的權限。",
        M::NoWritePermission => "你需要變更此伺服器檔案的權限。",
        M::NoSchedulePermission => "你需要變更此伺服器設定和檔案的權限。",
        M::LoadFailed => "無法載入備份：{error}",
        M::Schedule => "排程",
        M::Automatic => "自動備份",
        M::AutomaticHint => "如果自上次備份以來沒有任何變更，就不會儲存。",
        M::NextBackupIn => "{time}後進行下一次備份",
        M::NextBackupSoon => "即將進行下一次備份",
        M::NextBackupWhenPlayerJoins => "有玩家加入後將進行下一次備份",
        M::Interval => "頻率",
        M::EveryHour => "每小時",
        M::EveryDay => "每天",
        M::OnlyWhenActive => "僅在有玩家上線時",
        M::OnlyWhenActiveHint => "如果自上次備份以來沒有人玩過，就略過備份。",
        M::Keep => "保留",
        M::KeepHint => "保留多少個自動備份。手動備份和已釘選的備份一律保留。",
        M::KeepFewer => "較少",
        M::KeepNormal => "一般",
        M::KeepMore => "較多",
        M::KeepCustom => "自訂",
        M::ScheduleSaveFailed => "無法儲存排程：{error}",
        M::StorageFiles => "檔案共 {size}",
        M::StorageStored => "實際佔用 {size}",
        M::StorageFree => "剩餘 {size}",
        M::Empty => "還沒有備份。立即備份，或開啟自動備份。",
        M::DateAt => "{date} {time}",
        M::SizeNew => "{size} · 新增 {new}",
        M::TagManual => "手動",
        M::TagAutomatic => "自動",
        M::TagBeforeRestore => "還原前",
        M::TagLive => "執行中",
        M::TagLiveHint => "在伺服器執行時備份。",
        M::TagInconsistent => "可能不一致",
        M::TagInconsistentHint => {
            "伺服器在複製前沒有確認已儲存世界，因此世界可能是在儲存途中被複製的。"
        }
        M::TagSkippedHint => {
            "無法讀取的檔案（例如使用中的檔案）已被略過，或沿用了上一個備份中的版本。"
        }
        M::TagPinned => "已釘選",
        M::TagPinnedHint => "自動清理永遠不會刪除它。",
        M::RestoreAction => "還原…",
        M::DownloadLabel => "下載為 zip",
        M::PinLabel => "釘選，讓它永遠不會被自動刪除",
        M::UnpinLabel => "取消釘選",
        M::RestoreAsNewLabel => "還原為新伺服器",
        M::DeleteLabel => "刪除備份",
        M::RestoreTitle => "還原「{name}」",
        M::ScopeEverything => "全部",
        M::ScopeWorld => "僅世界",
        M::ScopeEverythingHint => {
            "以備份中的檔案取代伺服器的檔案。伺服器的名稱、連接埠和 Java 設定保持不變。"
        }
        M::ScopeWorldHint => "取代世界及其地獄和終界。設定、模組和外掛保持不變。",
        M::NoWorld => "此備份沒有世界資料夾，因此只能還原整個伺服器。",
        M::StopFirst => "先停止伺服器",
        M::StartAfter => "完成後重新啟動",
        M::MustStop => "必須先停止伺服器才能還原備份。",
        M::SafetyNote => "會先為伺服器目前的狀態做一個備份，讓你可以復原此操作。",
        M::Restore => "還原",
        M::RestoreStarted => "正在還原「{name}」…",
        M::DeleteTitle => "刪除此備份？",
        M::DeleteBody => "「{name}」將被刪除。伺服器本身不會改變。",
        M::Deleted => "備份已刪除",
        M::NewTitle => "還原為新伺服器",
        M::NewBody => "以此備份建立一個使用獨立連接埠的新伺服器。此伺服器不會改變。",
        M::NewName => "名稱",
        M::NewDefaultName => "{name}（已還原）",
        M::NewCreate => "建立伺服器",
        M::NewStarted => "正在建立「{name}」…",
        M::Zipping => "正在壓縮備份以供下載…",
        M::Downloaded => "已儲存 {name}",
        M::DownloadFailed => "無法下載備份：{error}",
        M::BackupDone => "備份完成",
        M::BackupFailed => "備份失敗",
        M::RestoreDone => "還原完成",
        M::RestoreFailed => "還原失敗",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} 個備份", "{count} 個備份"],
        P::Skipped => ["略過了 {count} 個檔案", "略過了 {count} 個檔案"],
        P::EveryMinutes => ["每 {count} 分鐘", "每 {count} 分鐘"],
        P::EveryHours => ["每 {count} 小時", "每 {count} 小時"],
        P::EveryDays => ["每 {count} 天", "每 {count} 天"],
    }
}
