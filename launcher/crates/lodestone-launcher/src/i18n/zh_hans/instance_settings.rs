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
        M::MemoryChosenAutomatically => {
            "根据这台电脑和实例的模组选择。在你更改之前，它会随模组调整。"
        }
        M::WindowDefaultSize => "游戏默认大小",
        M::WindowFullscreen => "{size}，全屏",
        M::WindowSizeInvalid => "窗口大小需要以像素为单位的宽度和高度。",
        M::WindowSize => "窗口大小",
        M::WindowSizeGameDefault => "游戏默认",
        M::WindowSizeThisScreen => "此屏幕",
        M::WindowSizeCustom => "自定义",
        M::WindowWidth => "窗口宽度",
        M::WindowHeight => "窗口高度",
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

        M::AutoGroupServers => "服务器",
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
            "位于 {core}，该主机已从“Lodestone Host”页面移除。请在那里重新添加该主机以加入服务器。"
        }
        M::CoreRemoved => "主机已移除",
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
        M::BeforeGoingBack => "回退到“{name}”之前",
        M::RevertEllipsis => "恢复…",
        M::RevertTitle => "恢复到 {name}？",
        M::RevertBody => "所选部分会被快照中的内容替换。替换前会先为它们创建快照，以便恢复。",
        M::Revert => "恢复",
        M::ShareEllipsis => "分享…",
        M::ShareSnapshotTitle => "分享 {name}",
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
        M::SnapshotLaunchSettingsWarning => {
            "它的实例设置会改变游戏的启动方式：Java、Java 参数、环境变量或内存。这些设置也能运行程序，所以只有选择了“实例设置”才会使用它们。只为来自你信任的人的快照选择它。"
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
        M::FindCrashingMod => "查找导致游戏崩溃的模组",
        M::FindCrashingModHint => "多次启动游戏，每次关闭一半模组，直到只剩一个。",
        M::FindCrashingModButton => "查找…",
        M::FindCrashingModKeywords => "崩溃 冲突 损坏 不兼容 排查",
        M::Export => "导出",
        M::ExportHint => "导出为 .mrpack 文件。",
        M::ExportButton => "导出…",
        M::Exporting => "正在导出…",
        M::Duplicate => "创建副本",
        M::Duplicating => "正在创建副本…",
        M::HostOnCore => "在 Lodestone 上托管",
        M::HostHint => "用此实例的模组创建服务器。",
        M::HostButton => "托管…",
        M::ShareCodeRow => "分享码",
        M::ShareButton => "分享…",
        M::ShareTitle => "分享 {name}",
        M::ShareIntro => {
            "另一个 Lodestone Launcher 可以用这个分享码创建同样的实例：相同的 Minecraft 版本、加载器和 Modrinth 内容。"
        }
        M::ShareReading => "正在读取内容…",
        M::ShareNotSharedBody => "分享码只包含来自 Modrinth 的内容。如有需要，请另行发送这些文件。",
        M::ShareFromCurseForge => "来自 CurseForge",
        M::ShareAddedByHand => "手动添加",
        M::ShareNotOnModrinth => "不在 Modrinth 上",
        M::ShareChangedSincePack => "自整合包安装后已更改",
        M::ShareUnchecked => "未向 Modrinth 核对",
        M::ShareBasedOnPack => "基于整合包 {pack}",
        M::ShareIntroPack => {
            "另一个 Lodestone Launcher 可以用这个分享码创建同样的实例：先安装整合包，再应用你的更改。"
        }
        M::ShareNotSharedBodyPack => {
            "整合包自带的文件会随之而来，但这些文件不会原样到达。如有需要，请另行发送。"
        }
        M::ShareNoChanges => "未更改整合包",
        M::ShareChanges => "你的更改：{changes}",
        M::CodePackPreview => "来自 Modrinth 的 {pack}",
        M::ShareMore => "还有 {count} 个",
        M::CopyCode => "复制分享码",
        M::CodeCopied => "已复制",
        M::ShareCodeMode => "分享码",
        M::PasteShareCode => "粘贴分享码",
        M::CodeNotACode => "这不是 Lodestone 分享码。",
        M::CodeNewer => "此分享码来自更新版本的 Lodestone Launcher。",
        M::CodeTooLarge => "分享码太长。",
        M::CodeDamaged => "分享码已损坏，请重新复制。",
        M::DeleteInstance => "删除实例",
        M::DeleteHint => "包括其中的世界和模组。",
        M::DeleteButton => "删除…",
        M::SharedLead => "让各实例的设置保持同步。",
        M::UseSharedSettings => "使用共享设置",
        M::SharingOff => "此实例保留自己的选项。",
        M::SharingFrom => "接收来自 {name} 的游戏内更改。",
        M::SharingMain => "此实例中游戏内的更改会同步到所有实例。",
        M::SharingNoMain => "没有主实例：共享值只能在设置中更改。",
        M::MainInstance => "主实例",
        M::MainInstanceHint => {
            "你在主实例游戏中更改的设置，会在其他共享设置的实例下次启动时复制过去。只能有一个主实例。"
        }
        M::IsMainInstance => "这是你的主实例",
        M::MakeMain => "设为主实例",
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
        M::MemoryKeywords => "RAM 运行内存 Xmx Xms",
        M::JavaKeywords => "JDK JRE 运行时 路径",
        M::JvmArgumentsKeywords => "Java 参数 选项",
        M::WindowKeywords => "分辨率 宽度 高度 全屏 尺寸",
        M::EnvironmentKeywords => "env 环境变量 PATH",
        M::PlayBackgroundKeywords => "壁纸 图片 背景 幻灯片",
        M::DeleteKeywords => "删除 移除 卸载",
        M::LoaderKeywords => "模组加载器 Fabric Forge NeoForge Quilt",

        M::AddModpackTitle => "向 {name} 添加整合包",
        M::AddModpackFilter => "适用于 Minecraft {version}（{loader}）的整合包。{note}",
        M::AddModpackServerNote => {
            "整合包的模组会与 {server} 的模组放在一起，服务器的模组保持不变，因此你仍然可以加入。"
        }
        M::AddModpackNote => {
            "整合包的文件会与实例自己的文件放在一起，不会替换实例中已有的任何文件。"
        }
        M::AddModpackButton => "添加整合包",
        M::CheckingModpack => "正在检查整合包",
        M::AddModpackSummary => "{pack} {version}：需下载 {size}。",
        M::AddModpackNothing => "此实例已包含该整合包的所有文件。",
        M::AddGroupFiles => "将添加（{count}）",
        M::AddGroupConfigs => "将添加的配置（{count}）",
        M::AddGroupServer => "保持 {server} 的版本（{count}）",
        M::AddGroupPack => "保持实例整合包的版本（{count}）",
        M::AddGroupThere => "实例中已有（{count}）",
        M::AddedModpacks => "已添加的整合包",
        M::AddModpackRow => "添加整合包",
        M::AddModpackHint => "将整合包的模组和配置添加到此实例中，与已有的放在一起。",
        M::AddModpackServerHint => {
            "将整合包的模组（例如客户端优化）添加到 {server} 的模组旁边。服务器的模组保持不变。"
        }
        M::AddModpackEllipsis => "添加整合包…",
        M::RemoveAddedModpackNote => "移除该整合包添加的文件。你修改过的文件会保留。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShareNotShared => ["有 {count} 个文件不会被分享"; 2],
        P::ShareAddedCount => ["添加 {count} 项"; 2],
        P::ShareRemovedCount => ["移除 {count} 项"; 2],
        P::ShareChangedCount => ["更改 {count} 项"; 2],
        P::ShareToggledCount => ["启用或禁用 {count} 项"; 2],
        P::CodePackChanges => ["+ {count} 项更改"; 2],
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

        P::AndMoreFiles => ["以及另外 {count} 个"; 2],
        P::AddedModpackFiles => ["已添加 {count} 个文件"; 2],
    }
}
