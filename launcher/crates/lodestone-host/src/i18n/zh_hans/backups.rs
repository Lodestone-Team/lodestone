use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "每个备份都是服务器的完整副本，但未改动的文件只存储一次，所以经常备份也不占多少空间。"
        }
        M::BackUpNow => "立即备份",
        M::BackingUp => "正在备份…",
        M::Restoring => "正在恢复…",
        M::JobRunning => "此服务器正在进行备份或恢复。",
        M::NoSettingsPermission => "你需要更改此服务器设置的权限。",
        M::NoWritePermission => "你需要更改此服务器文件的权限。",
        M::NoSchedulePermission => "你需要更改此服务器设置和文件的权限。",
        M::LoadFailed => "无法加载备份：{error}",
        M::Schedule => "计划",
        M::Automatic => "自动备份",
        M::AutomaticHint => "如果自上次备份以来没有任何改动，则不会保存。",
        M::NextBackupIn => "{time}后进行下一次备份",
        M::NextBackupSoon => "即将进行下一次备份",
        M::NextBackupWhenPlayerJoins => "有玩家加入后将进行下一次备份",
        M::Interval => "频率",
        M::EveryHour => "每小时",
        M::EveryDay => "每天",
        M::OnlyWhenActive => "仅在有玩家上线时",
        M::OnlyWhenActiveHint => "如果自上次备份以来没有人玩过，则跳过备份。",
        M::Keep => "保留",
        M::KeepHint => "保留多少个自动备份。手动备份和已固定的备份始终保留。",
        M::KeepFewer => "较少",
        M::KeepNormal => "正常",
        M::KeepMore => "较多",
        M::KeepCustom => "自定义",
        M::ScheduleSaveFailed => "无法保存计划：{error}",
        M::StorageFiles => "文件共 {size}",
        M::StorageStored => "实际占用 {size}",
        M::StorageFree => "剩余 {size}",
        M::Empty => "还没有备份。立即备份，或开启自动备份。",
        M::DateAt => "{date} {time}",
        M::SizeNew => "{size} · 新增 {new}",
        M::TagManual => "手动",
        M::TagAutomatic => "自动",
        M::TagBeforeRestore => "恢复前",
        M::TagLive => "运行中",
        M::TagLiveHint => "在服务器运行时备份。",
        M::TagInconsistent => "可能不一致",
        M::TagInconsistentHint => {
            "服务器在复制前没有确认已保存世界，因此世界可能是在保存过程中被复制的。"
        }
        M::TagSkippedHint => {
            "无法读取的文件（例如正在使用的文件）已被略过，或沿用了上一个备份中的版本。"
        }
        M::TagPinned => "已固定",
        M::TagPinnedHint => "自动清理永远不会删除它。",
        M::RestoreAction => "恢复…",
        M::DownloadLabel => "下载为 zip",
        M::PinLabel => "固定，使其永远不会被自动删除",
        M::UnpinLabel => "取消固定",
        M::RestoreAsNewLabel => "恢复为新服务器",
        M::DeleteLabel => "删除备份",
        M::RestoreTitle => "恢复“{name}”",
        M::ScopeEverything => "全部",
        M::ScopeWorld => "仅世界",
        M::ScopeEverythingHint => {
            "用备份中的文件替换服务器的文件。服务器的名称、端口和 Java 设置保持不变。"
        }
        M::ScopeWorldHint => "替换世界及其下界和末地。设置、模组和插件保持不变。",
        M::NoWorld => "此备份没有世界文件夹，因此只能恢复整个服务器。",
        M::StopFirst => "先停止服务器",
        M::StartAfter => "完成后重新启动",
        M::MustStop => "必须先停止服务器才能恢复备份。",
        M::SafetyNote => "会先为服务器的当前状态做一个备份，以便你撤销此操作。",
        M::Restore => "恢复",
        M::RestoreStarted => "正在恢复“{name}”…",
        M::DeleteTitle => "删除此备份？",
        M::DeleteBody => "“{name}”将被删除。服务器本身不会改变。",
        M::Deleted => "备份已删除",
        M::NewTitle => "恢复为新服务器",
        M::NewBody => "用此备份创建一个使用独立端口的新服务器。此服务器不会改变。",
        M::NewName => "名称",
        M::NewDefaultName => "{name}（已恢复）",
        M::NewCreate => "创建服务器",
        M::NewStarted => "正在创建“{name}”…",
        M::Zipping => "正在压缩备份以供下载…",
        M::Downloaded => "已保存 {name}",
        M::DownloadFailed => "无法下载备份：{error}",
        M::BackupDone => "备份完成",
        M::BackupFailed => "备份失败",
        M::RestoreDone => "恢复完成",
        M::RestoreFailed => "恢复失败",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} 个备份", "{count} 个备份"],
        P::Skipped => ["略过了 {count} 个文件", "略过了 {count} 个文件"],
        P::EveryMinutes => ["每 {count} 分钟", "每 {count} 分钟"],
        P::EveryHours => ["每 {count} 小时", "每 {count} 小时"],
        P::EveryDays => ["每 {count} 天", "每 {count} 天"],
    }
}
