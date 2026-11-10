use crate::i18n::{WorldsMsg as M, WorldsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Worlds => "世界",
        M::Screenshots => "截图",

        M::StopGameFirst => "请先停止游戏：游戏正在使用这个世界，并会在退出时保存它。",
        M::WaitForTask => "请等待当前任务完成。",
        M::Cancelled => "已取消。",
        M::GameRunning => "游戏正在运行：可以打开世界、查看备份，但更改要等游戏停止后才能进行。",

        M::BackingUp => "正在备份 {name}",
        M::RestoringBackup => "正在恢复备份",
        M::ReplacingWorld => "正在用备份替换世界",
        M::MovingToTrash => "正在将 {name} 移到{trash}",
        M::MovingBackupToTrash => "正在将备份移到{trash}",
        M::Renaming => "正在重命名",
        M::Exporting => "正在导出 {name}",
        M::ImportingWorlds => "正在导入世界",

        M::BackedUp => "已备份 {name}（{size}）。",
        M::RestoredAsCopy => "已恢复为新世界 {folder}。",
        M::Restored => "已恢复 {folder}。",
        M::RestoredWithSafety => "已恢复 {folder}。原来的世界已备份为 {backup}，并已移到{trash}。",
        M::InTrash => "{name} 已移到{trash}。",
        M::RenamedTo => "已重命名为 {name}。",
        M::RenamedToFolder => "已重命名为 {name}（文件夹 {folder}）。",
        M::Exported => "已将 {name} 导出到 {path}（{size}）。",
        M::Imported => "已导入 {worlds}。",
        M::FileError => "{name}：{error}",
        M::ZipsOnly => "世界只能从 .zip 文件导入。",
        M::ImportWorldPrompt => "导入世界",
        M::CopiedScreenshot => "已复制截图。",
        M::CouldntCopy => "无法复制：{error}",

        M::Hardcore => "极限",
        M::Survival => "生存",
        M::Creative => "创造",
        M::Adventure => "冒险",
        M::Spectator => "旁观",
        M::Cheats => "允许作弊",
        M::Snapshot => "{version}（快照）",
        M::PlayedAt => "上次游玩：{time}",
        M::BackUp => "创建备份",
        M::Backups => "备份",
        M::Export => "导出…",
        M::DeleteWorld => "删除…",
        M::NoBackupsYet => {
            "还没有备份。“创建备份”会将世界保存为 zip 文件，放在实例的 backups 文件夹中。"
        }
        M::RestoreAsCopy => "恢复为副本",
        M::ReplaceWorld => "替换世界…",

        M::DataPacks => "数据包",
        M::ReadingDataPacks => "正在读取数据包",
        M::NoDataPacksYet => {
            "这个世界还没有数据包。添加一个 .zip 文件或文件夹，或在下方从 Modrinth 查找。"
        }
        M::AddDataPack => "添加数据包…",
        M::AddDataPackPrompt => "添加数据包",
        M::SearchDataPacks => "在 Modrinth 上搜索数据包，或粘贴 Modrinth 链接",
        M::NewPack => "新",
        M::NewPackHint => "游戏会在下次加载这个世界时启用它。",
        M::PacksNotSwitchable => {
            "请先用 Minecraft 1.13 或更高版本打开这个世界一次，之后就能在这里启用和禁用它的数据包。"
        }
        M::AddingDataPacks => "正在添加数据包",
        M::SavingPackList => "正在保存世界的数据包列表",
        M::AddedDataPacks => "已将 {packs} 添加到 {name}。",
        M::InstalledDataPack => "已将 {pack} 安装到 {name}。",
        M::UpdatedDataPack => "已将 {pack} 安装到 {name}，替换了 {old}。",
        M::PackTurnedOn => "{pack} 已启用，会在下次打开世界时加载。",
        M::PackTurnedOff => "{pack} 已禁用，下次打开世界时不再加载。",
        M::NoDataPackVersion => "{name} 没有适用于 Minecraft {version} 的数据包。",
        M::SearchingModrinth => "正在搜索 Modrinth",
        M::NoDataPacksFound => "没有找到适用于 Minecraft {version} 的数据包。",
        M::LinkNotDataPack => "这个链接不是数据包。",
        M::ModrinthUnavailable => "现在无法连接 Modrinth；仍然可以从文件添加数据包。",

        M::ImportWorld => "导入世界…",
        M::OpenSavesFolder => "打开 saves 文件夹",
        M::ReadingWorlds => "正在读取世界",
        M::NoWorldsYet => "还没有世界",
        M::NoWorldsHint => {
            "你在游戏中创建的世界会显示在这里。你也可以导入世界 zip 文件，或将其拖放到这里。"
        }

        M::ReadingScreenshots => "正在读取截图",
        M::NoScreenshotsYet => "还没有截图",
        M::NoScreenshotsHint => "在游戏中按 F2 即可截图，截图会显示在这里。",
        M::Position => "第 {index} 张，共 {count} 张",
        M::CopyImage => "复制图片",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScreenshotsSummary => ["{count} 张截图 · 在游戏中按 F2 即可截图"; 2],
    }
}
