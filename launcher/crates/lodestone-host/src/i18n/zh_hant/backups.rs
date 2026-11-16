use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "每個快照都是伺服器資料夾的完整副本，但未變更的檔案只儲存一次，所以經常建立快照也不佔多少空間。"
        }
        M::BackUpNow => "建立快照",
        M::BackingUp => "正在建立快照…",
        M::Restoring => "正在還原…",
        M::JobRunning => "此伺服器正在進行快照、世界備份或還原。",
        M::NoSettingsPermission => "你需要變更此伺服器設定的權限。",
        M::NoWritePermission => "你需要變更此伺服器檔案的權限。",
        M::NoSchedulePermission => "你需要變更此伺服器設定和檔案的權限。",
        M::LoadFailed => "無法載入快照：{error}",
        M::Schedule => "排程",
        M::Automatic => "自動建立快照",
        M::AutomaticHint => "如果自上次快照以來沒有任何變更，則不會儲存。",
        M::NextBackupIn => "{time}後建立下一個快照",
        M::NextBackupSoon => "即將建立下一個快照",
        M::NextBackupWhenPlayerJoins => "有玩家加入後將建立下一個快照",
        M::Interval => "頻率",
        M::EveryHour => "每小時",
        M::EveryDay => "每天",
        M::OnlyWhenActive => "僅在有玩家上線時",
        M::OnlyWhenActiveHint => "如果自上次快照以來沒有人玩過，則略過快照。",
        M::Keep => "保留",
        M::KeepHint => "保留多少個自動快照。手動快照和已釘選的快照一律保留。",
        M::KeepFewer => "較少",
        M::KeepNormal => "一般",
        M::KeepMore => "較多",
        M::KeepCustom => "自訂",
        M::ScheduleSaveFailed => "無法儲存排程：{error}",
        M::StorageFiles => "檔案共 {size}",
        M::StorageStored => "實際佔用 {size}",
        M::StorageFree => "剩餘 {size}",
        M::Empty => "還沒有快照。立即建立一個，或開啟自動快照。",
        M::DateAt => "{date} {time}",
        M::SizeNew => "{size} · 新增 {new}",
        M::TagManual => "手動",
        M::TagAutomatic => "自動",
        M::TagBeforeRestore => "還原前",
        M::TagLive => "執行中",
        M::TagLiveHint => "在伺服器執行時建立。",
        M::TagInconsistent => "可能不一致",
        M::TagInconsistentHint => {
            "伺服器在複製前沒有確認已儲存世界，因此世界可能是在儲存途中被複製的。"
        }
        M::TagSkippedHint => {
            "無法讀取的檔案（例如使用中的檔案）已被略過，或沿用了上一個快照中的版本。"
        }
        M::TagPinned => "已釘選",
        M::TagPinnedHint => "自動清理永遠不會刪除它。",
        M::RestoreAction => "還原…",
        M::DownloadLabel => "下載為 zip",
        M::PinLabel => "釘選，讓它永遠不會被自動刪除",
        M::UnpinLabel => "取消釘選",
        M::RestoreAsNewLabel => "還原為新伺服器",
        M::DeleteLabel => "刪除快照",
        M::RestoreTitle => "還原「{name}」",
        M::ScopeEverything => "全部",
        M::ScopeWorld => "僅世界",
        M::ScopeEverythingHint => {
            "用快照中的檔案取代伺服器的檔案。伺服器的名稱、連接埠和 Java 設定保持不變。"
        }
        M::ScopeWorldHint => "取代世界及其地獄和終界。設定、模組和外掛保持不變。",
        M::NoWorld => "此快照沒有世界資料夾，因此只能還原整個伺服器。",
        M::StopFirst => "先停止伺服器",
        M::StartAfter => "完成後重新啟動",
        M::MustStop => "必須先停止伺服器才能還原快照。",
        M::SafetyNote => "會先為伺服器目前的狀態建立一個快照，讓你可以復原此操作。",
        M::Restore => "還原",
        M::RestoreStarted => "正在還原「{name}」…",
        M::DeleteTitle => "刪除此快照？",
        M::DeleteBody => "「{name}」將被刪除。伺服器本身不會改變。",
        M::Deleted => "快照已刪除",
        M::NewTitle => "還原為新伺服器",
        M::NewBody => "用此快照建立一個使用獨立連接埠的新伺服器。此伺服器不會改變。",
        M::NewName => "名稱",
        M::NewDefaultName => "{name}（已還原）",
        M::NewCreate => "建立伺服器",
        M::NewStarted => "正在建立「{name}」…",
        M::Zipping => "正在壓縮快照以供下載…",
        M::Downloaded => "已儲存 {name}",
        M::DownloadFailed => "無法下載快照：{error}",
        M::BackupDone => "快照完成",
        M::BackupFailed => "快照失敗",
        M::RestoreDone => "還原完成",
        M::RestoreFailed => "還原失敗",
        M::ExportLabel => "匯出為檔案，供其他 Host 或 Launcher 使用",
        M::ExportTitle => "匯出「{name}」",
        M::ExportBody => {
            "將此快照儲存為 .lodeimg 檔案。其他 Lodestone Host 可以用它建立伺服器，Lodestone Launcher 可以用它的世界建立單人遊戲實例。server.properties 中的密碼會被清空，含有密碼或權杖的檔案會被排除。"
        }
        M::ExportPlugins => "包含外掛設定（其中可能有密碼）",
        M::ExportPluginsHint => "外掛資料夾中的全部內容：設定、資料庫和玩家資料。",
        M::ExportAction => "匯出…",
        M::ExportPreparing => "正在準備檔案…",
        M::ExportSaving => "正在儲存檔案…",
        M::Exported => "已儲存 {name}",
        M::ExportedBlanked => "已清空：{names}",
        M::ExportedNotDownloadable => "未包含在檔案中，因為其作者不允許在別處下載：{names}",
        M::ExportedSecretsWithheld => {
            "已排除，因為其中含有密碼或權杖（另一台伺服器會寫入預設設定）：{names}"
        }
        M::ExportFailed => "無法匯出快照：{error}",
        M::ExportCancelled => "匯出已取消",
        M::WorldSection => "世界備份",
        M::SnapshotsSection => "快照",
        M::WorldLead => {
            "世界備份是世界及其地獄和終界的一般 .zip 檔案，也可以在 Lodestone 之外開啟。只有在你要求時才會建立。"
        }
        M::WorldBackUp => "備份世界…",
        M::WorldCreateTitle => "備份世界",
        M::WorldCreateBody => "將世界儲存為 .zip。執行中的伺服器會繼續執行；複製世界時會暫停儲存。",
        M::WorldName => "名稱（選填）",
        M::WorldCreate => "備份",
        M::WorldBackingUp => "正在備份世界…",
        M::WorldRestoring => "正在還原世界…",
        M::WorldEmpty => "還沒有世界備份。",
        M::WorldLoadFailed => "無法載入世界備份：{error}",
        M::WorldSizeVersion => "{size} · Minecraft {version}",
        M::WorldBeforeRestoreHint => "在還原「{name}」之前建立。",
        M::WorldTagInconsistentHint => "伺服器在複製前沒有確認已儲存世界。",
        M::WorldLeftOutHint => "連結和無法讀取的檔案已被略過。",
        M::WorldDownloadLabel => "下載 .zip",
        M::WorldDeleteLabel => "刪除世界備份",
        M::WorldRestoreTitle => "從「{name}」還原世界",
        M::WorldRestoreBody => {
            "用備份中的世界及其地獄和終界取代此伺服器的世界。設定、模組和外掛保持不變。"
        }
        M::WorldMustStop => "必須先停止伺服器才能還原世界備份。",
        M::WorldSafetyNote => "會先將目前的世界儲存為一個世界備份，讓你可以復原此操作。",
        M::WorldRestoreStarted => "正在從「{name}」還原世界…",
        M::WorldDeleteTitle => "刪除此世界備份？",
        M::WorldDeleteBody => "「{name}」將被刪除。伺服器的世界不會改變。",
        M::WorldDeleted => "世界備份已刪除",
        M::WorldDownloadFailed => "無法下載世界備份：{error}",
        M::WorldDownloadCancelled => "下載已取消",
        M::WorldBackupDone => "世界備份完成",
        M::WorldBackupFailed => "世界備份失敗",
        M::WorldRestoreDone => "世界已還原",
        M::WorldRestoreFailed => "無法還原世界",
        M::WorldCancelFailed => "無法取消：{error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} 個快照", "{count} 個快照"],
        P::Skipped => ["略過了 {count} 個檔案", "略過了 {count} 個檔案"],
        P::EveryMinutes => ["每 {count} 分鐘", "每 {count} 分鐘"],
        P::EveryHours => ["每 {count} 小時", "每 {count} 小時"],
        P::EveryDays => ["每 {count} 天", "每 {count} 天"],
        P::ExportedLeftOut => [
            "已移除 {count} 個不能離開這台電腦的檔案",
            "已移除 {count} 個不能離開這台電腦的檔案",
        ],
        P::WorldCount => ["{count} 個世界備份", "{count} 個世界備份"],
        P::WorldLeftOut => ["略過了 {count} 個檔案", "略過了 {count} 個檔案"],
    }
}
