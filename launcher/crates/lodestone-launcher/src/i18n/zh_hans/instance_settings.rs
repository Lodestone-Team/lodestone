use crate::i18n::{InstanceSettingsMsg as M, InstanceSettingsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::MemoryFormat => "请以 MB（如 4096）或 GB（如 4G）为单位填写内存，至少 256 MB。",
        M::MemoryMinAboveMax => "最小值大于最大值。",
        // "这台 Mac" ends in Latin letters, "这台电脑" doesn't.
        M::MemoryTooHigh => {
            if cfg!(target_os = "macos") {
                "这超过了{computer} 内存的四分之三，留给 {os} 和其他应用的内存会太少。"
            } else {
                "这超过了{computer}内存的四分之三，留给 {os} 和其他应用的内存会太少。"
            }
        }
        M::TotalMemory => {
            if cfg!(target_os = "macos") {
                "{computer} 有 {size} 内存。"
            } else {
                "{computer}有 {size} 内存。"
            }
        }
        M::MemoryRange => "{min} – {max}",
        M::MemoryUnit => "MB",
        M::WindowDefaultSize => "游戏默认大小",
        M::WindowFullscreen => "{size}，全屏",
        M::WindowSizeInvalid => "窗口大小需要以像素为单位的宽度和高度。",
        M::EnvInvalid => "环境变量：{error}",
        M::EnvPlaceholder => "名称=值",

        M::General => "常规",
        M::GameAndLoader => "游戏与加载器",
        M::JavaAndMemory => "Java 与内存",
        M::LaunchOptions => "启动选项",
        M::SharedGameSettings => "共享游戏设置",
        M::Maintenance => "维护",
        M::Modpack => "整合包",
        M::LinkedServer => "关联的服务器",

        M::Name => "名称",
        M::Group => "分组",
        M::NoGroup => "无分组",
        M::NameMissing => "请为实例命名。",
        M::Icon => "图标",
        M::PlayBackground => "开始游戏页背景",
        M::UseAsIcon => "用作图标",
        M::UseAsBackground => "用作背景",
        M::SavedSeePlayTab => "已保存。到“开始游戏”标签页查看。",
        M::YourScreenshots => "你的截图",
        M::FindingScreenshots => "正在查找你的截图",
        M::NoScreenshots => "还没有截图。在游戏中按 F2 截图，或选择一个文件。",
        M::ChooseFile => "选择文件…",
        M::UseDefault => "使用默认",
        M::BackgroundDefault => "默认",
        M::BackgroundImage => "图片",
        M::BackgroundSlideshow => "幻灯片",
        M::SlideshowThisInstance => "此实例",
        M::SlideshowAllInstances => "所有实例",
        M::SlideshowOwnImages => "我的图片",
        M::AddImages => "添加图片…",
        M::AddToSlideshow => "添加到幻灯片",
        M::SlideshowNoScreenshots => "还没有截图。在有截图之前显示默认插图。",
        M::SlideshowNoImages => "还没有图片。添加一些，或在下方选择截图。",
        M::BlocksAndItems => "方块与物品",
        M::Emoji => "表情符号",
        M::UploadImage => "上传图片…",

        M::AutoGroupFabric => "Fabric（模组）",
        M::AutoGroupForge => "Forge（模组）",
        M::AutoGroupNeoForge => "NeoForge（模组）",
        M::AutoGroupQuilt => "Quilt（模组）",
        M::AutoGroupVanilla => "原版",
        M::AutoGroupImported => "已导入",

        M::GameVersionHint => "在创建实例时设置。",
        M::LoaderInstallsNextStart => "{loader} 将在游戏下次启动时安装。",
        M::LoaderInstallsNextPlay => "{loader} 将在你下次点击“开始游戏”时安装。",
        M::SavedNextStart => "已保存。将在游戏下次启动时生效。",
        M::CouldntTurnModsOff => "无法禁用模组：{error}",

        M::Vanilla => "原版",
        M::ModLoader => "模组加载器",
        M::AboutVanilla => "Mojang 发布的原版 Minecraft。",
        M::AboutFabric => "轻量、更新快。大多数性能模组。",
        M::AboutQuilt => "兼容 Fabric，并有自己的扩展。",
        M::AboutForge => "经典之选。大型内容模组和旧整合包。",
        M::AboutNeoForge => "Forge 的后继者，面向新版本。",
        M::PrereleaseUpdates => "测试版和早期测试版更新",
        M::PrereleaseUpdatesHint => "关闭时，已在使用测试版的模组仍会收到测试版更新。",
        M::LoaderVersion => "加载器版本",
        M::ChooseLoaderVersion => "请选择 {loader} 版本。",
        M::ChooseVersion => "选择版本",
        M::LoadingLoaderVersions => "正在加载 {loader} 版本",
        M::CouldntLoadVersions => "无法加载版本：{error}",
        M::NoVersionsFor => "没有适用于 Minecraft {game} 的版本。",
        M::Recommended => "推荐",
        M::Beta => "测试版",

        M::FromModrinth => "来自 Modrinth",
        M::FromServerMods => "来自 {server} 的模组：“开始游戏”会让它们与服务器保持一致。",
        M::FromMrpack => "来自不在 Modrinth 上的 .mrpack 文件，因此无法在此更新。",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::DownloadingNewVersion => "正在下载新版本…",
        M::CheckingModrinth => "正在检查 Modrinth…",
        M::UpdateTo => "更新到 {version}…",
        M::UpToDate => "已是最新",
        M::CheckForUpdates => "检查更新",
        M::ModrinthUnreachable => "无法连接到 Modrinth。",
        M::StopGameFirst => "请先停止游戏，或等待更新完成。",
        M::CouldntPrepareUpdate => "无法准备更新：{error}",

        M::LinkedServerHint => "位于 {core}。“开始游戏”会加入它并同步模组。",
        M::LinkedServerCoreRemoved => {
            "位于 {core}，该核心已从“服务器”页面移除。请在那里重新添加该核心以加入服务器。"
        }
        M::CoreRemoved => "核心已移除",
        M::Unlink => "取消关联",
        M::Unlinked => "已取消关联",
        M::UnlinkNote => "“开始游戏”将不再加入该服务器，模组会保留。",

        M::ChangeSetting => "更改",
        M::JavaMemoryLead => "默认值来自启动器设置。",
        M::Memory => "内存",
        M::Checking => "正在检查…",
        M::JavaFound => "Java {major}（版本 {version}）",
        M::JavaTooOld => {
            "Java {major}（版本 {version}）。Minecraft {game} 需要 Java {needed} 或更高版本。"
        }
        M::MojangJavaVersion => "Mojang 提供的 Java {major}",
        M::MojangJava => "Mojang 提供的 Java",
        M::UseThisJava => "使用此 Java",
        M::Check => "检查",
        M::JvmArguments => "JVM 参数",
        M::GameArguments => "游戏参数",
        M::NoArguments => "无",
        M::NoVariables => "无",
        M::Window => "窗口",
        M::Fullscreen => "全屏",
        M::EnvironmentVariables => "环境变量",

        M::InstanceSnapshots => "快照",
        M::TakeSnapshotEllipsis => "创建快照…",
        M::TakeSnapshotTitle => "创建快照",
        M::TakeSnapshot => "创建快照",
        M::SnapshotName => "名称",
        M::WhatToInclude => "包含的内容",
        M::WhatToRestore => "要恢复的内容",
        M::PartMods => "模组",
        M::PartResourcePacks => "资源包",
        M::PartShaderPacks => "光影包",
        M::PartConfigs => "配置和选项",
        M::PartWorlds => "世界",
        M::PartScreenshots => "截图",
        M::PartSettings => "实例设置",
        M::PartOther => "其他文件",
        M::NoSnapshots => "还没有快照。",
        M::SnapshotSizes => "{total}，其中新增 {new}",
        M::AutomaticSnapshot => "自动",
        M::RevertEllipsis => "恢复…",
        M::RevertTitle => "恢复到 {name}？",
        M::RevertBody => "所选部分会被快照中的内容替换。替换前会先为它们创建快照，以便恢复。",
        M::Revert => "恢复",
        M::ShareEllipsis => "分享…",
        M::ShareTitle => "分享 {name}",
        M::PreparingFile => "正在准备文件…",
        M::DragToShare => "将文件拖到其他应用中，或：",
        M::SaveAsEllipsis => "另存为…",
        M::ShowInFolder => "在文件夹中显示",
        M::DeleteSnapshotTitle => "删除 {name}？",
        M::DeleteSnapshotBody => "没有其他快照使用的文件会被删除。此操作无法撤销。",
        M::TakingSnapshot => "正在创建快照…",
        M::RevertingSnapshot => "正在恢复快照…",
        M::ImportingSnapshot => "正在创建实例…",
        M::ReadingSnapshot => "正在读取快照…",
        M::ImportSnapshotTitle => "从快照新建实例",
        M::SnapshotFrom => "{name} · Minecraft {version}",
        M::SnapshotModsWarning => {
            "其中包含 {count} 个模组：它们是游戏启动时会运行的程序。只使用来自你信任的人的快照。"
        }
        M::CreateInstanceButton => "创建实例",
        M::SnapshotGameRunning => "请先停止游戏。",
        M::ChooseSomething => "请至少选择一项。",
        M::SnapshotTaken => "快照已创建。",
        M::SnapshotReverted => "快照已恢复。",
        M::SnapshotsHint => "此实例的副本，可用于恢复或分享。",
        M::VerifyGameFiles => "校验游戏文件",
        M::VerifyHint => "重新下载缺失或损坏的文件。",
        M::Verify => "校验",
        M::VerifyStarted => "正在检查每个文件。进度显示在“开始游戏”按钮上。",
        M::Export => "导出",
        M::ExportHint => "导出为 .mrpack 文件。",
        M::ExportButton => "导出…",
        M::Exporting => "正在导出…",
        M::Duplicate => "创建副本",
        M::HostOnCore => "在 Lodestone Core 上托管",
        M::HostHint => "用此实例的模组创建服务器。",
        M::HostButton => "托管…",
        M::DeleteInstance => "删除实例",
        M::DeleteHint => "包括其中的世界和模组。",
        M::DeleteButton => "删除…",

        M::SharedLead => "所有实例使用相同的游戏选项。",
        M::UseSharedSettings => "使用共享设置",
        M::SharingOff => "此实例保留自己的选项。",
        M::SharingPreferred => "此实例中游戏内的更改会同步到所有实例。",
        M::SharingFrom => "接收来自 {name} 的游戏内更改。",
        M::SharingNoPreferred => "共享值只能在设置中更改。",
        M::Preferred => "首选",
        M::MakePreferred => "设为首选",
        M::PreferredExplained => {
            "在首选实例的游戏内更改的设置会成为共享设置；每个使用共享设置的实例会在下次启动时获得这些设置。只能有一个首选实例。"
        }
        M::MadePreferred => "此实例中的游戏内更改现在会更新到每个实例。",
        M::StopGameToChange => "请先停止游戏再更改：游戏退出时会重写 options.txt。",
        M::NotShared => "未共享（启动器设置）",
        M::ShareAll => "全部共享",
        M::OverrideAll => "全部覆盖",
        M::Show => "显示",
        M::Hide => "隐藏",
        M::NotSharedYet => "尚未共享",
        M::NotInOptions => "不在 options.txt 中",
        M::ThisInstance => "此实例",
        M::UseShared => "使用共享值（{value}）",
        M::Shared => "共享",
        M::Override => "覆盖",
        M::Value => "值",
        M::KeepForInstance => "为此实例保留",
        M::Dismiss => "忽略",
        M::SharedNotApplied => "未能应用共享游戏设置：{error}",
        M::SharedNotReadBack => "未能读回共享游戏设置：{error}",
        M::CategoryControls => "控制",
        M::CategoryVideo => "视频",
        M::CategorySound => "声音",
        M::CategoryAccessibility => "辅助功能与聊天",
        M::CategoryLanguage => "语言",
        M::CategoryControlsDescription => "鼠标、按键",
        M::CategoryVideoDescription => "视场角、渲染距离、界面尺寸、图像品质",
        M::CategorySoundDescription => "音量、字幕",
        M::CategoryAccessibilityDescription => "复述功能、聊天、文本背景",
        M::CategoryLanguageDescription => "游戏语言和字体",

        M::NewInstance => "新建实例",
        M::MinecraftVersion => "Minecraft 版本",
        M::MyInstance => "我的实例",
        M::SearchVersions => "搜索版本",
        M::Releases => "正式版",
        M::Snapshots => "快照版",
        M::OldVersions => "远古版本",
        M::KindRelease => "正式版",
        M::KindSnapshot => "快照版",
        M::KindBeta => "Beta 版",
        M::KindAlpha => "Alpha 版",
        M::KindOther => "其他",
        M::LoadingVersionList => "正在加载 Mojang 的版本列表",
        M::NoVersionsMatch => "没有匹配的版本。",
        M::PickModpack => "请选择整合包和版本。",
        M::ChooseGameVersion => "请选择 Minecraft 版本。",
        M::CreateInstance => "创建实例",
        M::InstallModpack => "安装整合包",
        M::ImportMrpack => "导入 .mrpack…",
        M::Import => "导入",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::LoaderNextStartModsOff => {
            ["{loader} 将在游戏下次启动时安装。已禁用 {count} 个模组。"; 2]
        }
        P::LoaderNextPlayModsOff => {
            ["{loader} 将在你下次点击“开始游戏”时安装。已禁用 {count} 个模组。"; 2]
        }
        P::SharedCount => ["{count} 项共享"; 2],
        P::OverriddenCount => ["{count} 项覆盖"; 2],
        P::AndMore => ["{names}及另外 {count} 项"; 2],
        P::ChangedInGame => {
            ["你在游戏中更改的 {count} 项设置（{names}）将在下次启动时被共享值替换。"; 2]
        }
        P::SharedWritten => ["共享游戏设置：已将 {count} 个值写入 options.txt"; 2],
        P::SharedWrittenNewFile => ["共享游戏设置：已将 {count} 个值写入新建的 options.txt"; 2],
        P::SharedEmitted => ["共享游戏设置：此实例的 {count} 个值现已应用到所有实例"; 2],
        P::SharedChangedNote => {
            ["共享游戏设置：游戏中更改的 {count} 项设置将在下次启动时被共享值替换（见“设置”）"; 2]
        }
    }
}
