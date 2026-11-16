use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "每个快照都是服务器文件夹的完整副本，但未改动的文件只存储一次，所以经常拍摄快照也不占多少空间。"
        }
        M::BackUpNow => "拍摄快照",
        M::BackingUp => "正在拍摄快照…",
        M::Restoring => "正在恢复…",
        M::JobRunning => "此服务器正在进行快照、世界备份或恢复。",
        M::NoSettingsPermission => "你需要更改此服务器设置的权限。",
        M::NoWritePermission => "你需要更改此服务器文件的权限。",
        M::NoSchedulePermission => "你需要更改此服务器设置和文件的权限。",
        M::LoadFailed => "无法加载快照：{error}",
        M::Schedule => "计划",
        M::Automatic => "自动拍摄快照",
        M::AutomaticHint => "如果自上次快照以来没有任何改动，则不会保存。",
        M::NextBackupIn => "{time}后拍摄下一个快照",
        M::NextBackupSoon => "即将拍摄下一个快照",
        M::NextBackupWhenPlayerJoins => "有玩家加入后将拍摄下一个快照",
        M::Interval => "频率",
        M::EveryHour => "每小时",
        M::EveryDay => "每天",
        M::OnlyWhenActive => "仅在有玩家上线时",
        M::OnlyWhenActiveHint => "如果自上次快照以来没有人玩过，则跳过快照。",
        M::Keep => "保留",
        M::KeepHint => "保留多少个自动快照。手动快照和已固定的快照始终保留。",
        M::KeepFewer => "较少",
        M::KeepNormal => "正常",
        M::KeepMore => "较多",
        M::KeepCustom => "自定义",
        M::ScheduleSaveFailed => "无法保存计划：{error}",
        M::StorageFiles => "文件共 {size}",
        M::StorageStored => "实际占用 {size}",
        M::StorageFree => "剩余 {size}",
        M::Empty => "还没有快照。立即拍摄一个，或开启自动快照。",
        M::DateAt => "{date} {time}",
        M::SizeNew => "{size} · 新增 {new}",
        M::TagManual => "手动",
        M::TagAutomatic => "自动",
        M::TagBeforeRestore => "恢复前",
        M::TagLive => "运行中",
        M::TagLiveHint => "在服务器运行时创建。",
        M::TagInconsistent => "可能不一致",
        M::TagInconsistentHint => {
            "服务器在复制前没有确认已保存世界，因此世界可能是在保存过程中被复制的。"
        }
        M::TagSkippedHint => {
            "无法读取的文件（例如正在使用的文件）已被略过，或沿用了上一个快照中的版本。"
        }
        M::TagPinned => "已固定",
        M::TagPinnedHint => "自动清理永远不会删除它。",
        M::RestoreAction => "恢复…",
        M::DownloadLabel => "下载为 zip",
        M::PinLabel => "固定，使其永远不会被自动删除",
        M::UnpinLabel => "取消固定",
        M::RestoreAsNewLabel => "恢复为新服务器",
        M::DeleteLabel => "删除快照",
        M::RestoreTitle => "恢复“{name}”",
        M::ScopeEverything => "全部",
        M::ScopeWorld => "仅世界",
        M::ScopeEverythingHint => {
            "用快照中的文件替换服务器的文件。服务器的名称、端口和 Java 设置保持不变。"
        }
        M::ScopeWorldHint => "替换世界及其下界和末地。设置、模组和插件保持不变。",
        M::NoWorld => "此快照没有世界文件夹，因此只能恢复整个服务器。",
        M::StopFirst => "先停止服务器",
        M::StartAfter => "完成后重新启动",
        M::MustStop => "必须先停止服务器才能恢复快照。",
        M::SafetyNote => "会先为服务器的当前状态拍摄一个快照，以便你撤销此操作。",
        M::Restore => "恢复",
        M::RestoreStarted => "正在恢复“{name}”…",
        M::DeleteTitle => "删除此快照？",
        M::DeleteBody => "“{name}”将被删除。服务器本身不会改变。",
        M::Deleted => "快照已删除",
        M::NewTitle => "恢复为新服务器",
        M::NewBody => "用此快照创建一个使用独立端口的新服务器。此服务器不会改变。",
        M::NewName => "名称",
        M::NewDefaultName => "{name}（已恢复）",
        M::NewCreate => "创建服务器",
        M::NewStarted => "正在创建“{name}”…",
        M::Zipping => "正在压缩快照以供下载…",
        M::Downloaded => "已保存 {name}",
        M::DownloadFailed => "无法下载快照：{error}",
        M::BackupDone => "快照完成",
        M::BackupFailed => "快照失败",
        M::RestoreDone => "恢复完成",
        M::RestoreFailed => "恢复失败",
        M::ExportLabel => "导出为文件，供其他 Host 或 Launcher 使用",
        M::ExportTitle => "导出“{name}”",
        M::ExportBody => {
            "将此快照保存为 .lodeimg 文件。其他 Lodestone Host 可以用它创建服务器，Lodestone Launcher 可以用它的世界创建单人游戏实例。server.properties 中的密码会被清空，含有密码或令牌的文件会被排除。"
        }
        M::ExportPlugins => "包含插件设置（其中可能有密码）",
        M::ExportPluginsHint => "插件文件夹中的全部内容：设置、数据库和玩家数据。",
        M::ExportAction => "导出…",
        M::ExportPreparing => "正在准备文件…",
        M::ExportSaving => "正在保存文件…",
        M::Exported => "已保存 {name}",
        M::ExportedBlanked => "已清空：{names}",
        M::ExportedNotDownloadable => "未包含在文件中，因为其作者不允许在别处下载：{names}",
        M::ExportedSecretsWithheld => {
            "已排除，因为其中含有密码或令牌（另一台服务器会写入默认设置）：{names}"
        }
        M::ExportFailed => "无法导出快照：{error}",
        M::ExportCancelled => "导出已取消",
        M::WorldSection => "世界备份",
        M::SnapshotsSection => "快照",
        M::WorldLead => {
            "世界备份是世界及其下界和末地的普通 .zip 文件，也可以在 Lodestone 之外打开。只有在你要求时才会创建。"
        }
        M::WorldBackUp => "备份世界…",
        M::WorldCreateTitle => "备份世界",
        M::WorldCreateBody => {
            "将世界保存为 .zip。正在运行的服务器会继续运行；复制世界时会暂停保存。"
        }
        M::WorldName => "名称（可选）",
        M::WorldCreate => "备份",
        M::WorldBackingUp => "正在备份世界…",
        M::WorldRestoring => "正在恢复世界…",
        M::WorldEmpty => "还没有世界备份。",
        M::WorldLoadFailed => "无法加载世界备份：{error}",
        M::WorldSizeVersion => "{size} · Minecraft {version}",
        M::WorldBeforeRestoreHint => "在恢复“{name}”之前创建。",
        M::WorldTagInconsistentHint => "服务器在复制前没有确认已保存世界。",
        M::WorldLeftOutHint => {
            "链接、特殊文件、无法读取的文件以及在某些磁盘上名称冲突的文件已被略过。"
        }
        M::WorldDownloadLabel => "下载 .zip",
        M::WorldDeleteLabel => "删除世界备份",
        M::WorldRestoreTitle => "从“{name}”恢复世界",
        M::WorldRestoreBody => {
            "用备份中的世界及其下界和末地替换此服务器的世界。设置、模组和插件保持不变。"
        }
        M::WorldMustStop => "必须先停止服务器才能恢复世界备份。",
        M::WorldSafetyNote => "会先将当前世界保存为一个世界备份，以便你撤销此操作。",
        M::WorldRestoreStarted => "正在从“{name}”恢复世界…",
        M::WorldDeleteTitle => "删除此世界备份？",
        M::WorldDeleteBody => "“{name}”将被删除。服务器的世界不会改变。",
        M::WorldDeleted => "世界备份已删除",
        M::WorldDownloadFailed => "无法下载世界备份：{error}",
        M::WorldDownloadCancelled => "下载已取消",
        M::WorldBackupDone => "世界备份完成",
        M::WorldBackupFailed => "世界备份失败",
        M::WorldRestoreDone => "世界已恢复",
        M::WorldRestoreFailed => "无法恢复世界",
        M::WorldCancelFailed => "无法取消：{error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} 个快照", "{count} 个快照"],
        P::Skipped => ["略过了 {count} 个文件", "略过了 {count} 个文件"],
        P::EveryMinutes => ["每 {count} 分钟", "每 {count} 分钟"],
        P::EveryHours => ["每 {count} 小时", "每 {count} 小时"],
        P::EveryDays => ["每 {count} 天", "每 {count} 天"],
        P::ExportedLeftOut => [
            "已移除 {count} 个不能离开这台电脑的文件",
            "已移除 {count} 个不能离开这台电脑的文件",
        ],
        P::WorldCount => ["{count} 个世界备份", "{count} 个世界备份"],
        P::WorldLeftOut => ["略过了 {count} 个文件", "略过了 {count} 个文件"],
    }
}
