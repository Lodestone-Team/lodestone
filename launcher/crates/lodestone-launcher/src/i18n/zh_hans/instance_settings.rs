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
        M::GameVersionFollowsModpack => "随整合包更新而变化。",
        M::GameVersionFollowsServer => "随关联服务器的整合包而变化。",
        M::LoaderInstallsNextStart => "{loader} 将在游戏下次启动时安装。",
        M::LoaderInstallsNextPlay => "{loader} 将在你下次点击“开始游戏”时安装。",
        M::SavedNextStart => "已保存。将在游戏下次启动时生效。",

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
        M::FromCurseForge => "来自 CurseForge",
        M::FromServerMods => "来自 {server} 的模组：“开始游戏”会让它们与服务器保持一致。",
        M::FromMrpack => "来自不在 Modrinth 上的 .mrpack 文件，因此无法在此更新。",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::ViewOnCurseForge => "在 CurseForge 上查看",
        M::DownloadingNewVersion => "正在下载新版本…",
        M::CheckingModrinth => "正在检查 Modrinth…",
        M::CheckingCurseForge => "正在检查 CurseForge…",
        M::UpdateModpack => "更新…",
        M::UpToDate => "已是最新",
        M::CheckForUpdates => "检查更新",
        M::ModrinthUnreachable => "无法连接到 Modrinth。",
        M::CurseForgeUnreachable => "无法连接到 CurseForge。",
        M::StopGameFirst => "请先停止游戏，或等待更新完成。",
        M::CouldntPrepareUpdate => "无法准备更新：{error}",

        M::LinkedServerHint => "“开始游戏”会加入它并同步其模组。",
        M::LinkedServerCoreRemoved => {
            "它的主机已从“Lodestone Host”页面中移除。请在那里重新添加该主机以加入服务器。"
        }
        M::CoreRemoved => "主机已移除",
        M::Unlink => "取消关联",
        M::Unlinked => "已取消关联",
        M::UnlinkNote => "“开始游戏”将不再加入该服务器，模组会保留。",

        M::ChangeSetting => "更改",
        M::JavaMemoryLead => "内存的默认值在“设置 › 实例默认设置”中设定。",
        M::Memory => "内存",
        M::Checking => "正在检查…",
        M::JavaTooOldBadge => "版本过旧",
        M::JavaFitsBadge => "可用",
        M::JavaFoundKey => "检测到",
        M::JavaVersion => "Java {version}",
        M::JavaOrNewer => "Java {major} 或更新版本",
        M::MojangJavaVersion => "Mojang 提供的 Java {major}",
        M::MojangJava => "Mojang 提供的 Java",
        M::AzulZulu8 => "适用于 Apple 芯片的 Azul Zulu 8",
        M::MojangIntelJavaVersion => "Mojang 提供的 Intel 版 Java {major}，通过 Rosetta 2 运行",
        M::JavaTheVersionNeeds => "此版本所需的 Java",
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
        M::NoSnapshots => "还没有快照。",
        M::SnapshotSizes => "{total}，其中新增 {new}",
        M::SnapshotNothingNew => "{total}，无新增",
        M::PreparingFile => "正在准备文件…",
        M::DragToShare => "将文件拖到其他应用中，或：",
        M::SaveAsEllipsis => "另存为…",
        M::ShowInFolder => "在文件夹中显示",
        M::DeleteSnapshotTitle => "删除 {name}？",
        M::DeleteSnapshotBody => {
            "快照会被删除。它的文件会在下次清理时删除，除非其他快照也在使用。此操作无法撤销。"
        }
        M::TakingSnapshot => "正在创建快照…",
        M::ImportingSnapshot => "正在创建实例…",
        M::ReadingSnapshot => "正在读取快照…",
        M::ImportSnapshotTitle => "从快照新建实例",
        M::ImportServerTitle => "从服务器的世界新建实例",
        M::CreateInstanceButton => "创建实例",
        M::SnapshotGameRunning => "请先停止游戏。",
        M::SnapshotAfterPlay => "每次游戏结束后拍摄快照",
        M::BadgeAutomatic => "自动",
        M::BadgeAfterPlaying => "游戏后",
        M::BadgeImported => "已导入",
        M::BadgePinned => "已固定",
        M::RestoreEllipsis => "恢复…",
        M::ExportEllipsis => "导出…",
        M::PinSnapshot => "固定",
        M::UnpinSnapshot => "取消固定",
        M::UnreadableImagesHint => "有无法读取的镜像时，旧的内容历史不会被清理。",
        M::DeleteUnreadable => "删除它们",
        M::RestoreTitle => "恢复 {name}",
        M::RestoreWorldsOnly => "仅世界",
        M::RestoreWorldsAndMods => "世界及其模组",
        M::RestoreEverything => "全部",
        M::RestoreAsNew => "作为新实例",
        M::RestoreAsNewName => "新实例名称",
        M::RestoreEverythingHint => {
            "整个游戏文件夹恢复到快照时的状态：世界、模组、配置和选项。之后创建的世界会被删除。"
        }
        M::WorldGoneNow => "{size} · 实例中已没有",
        M::ModsDiffer => "这些世界上次运行时使用的是其他模组。",
        M::FilesDiffer => "这些世界上次运行时使用的是其他文件。",
        M::RestoreModsToo => "同时恢复它们的模组",
        M::RestoreModsTooChanges => "同时恢复它们的模组将会：",
        M::RestoreLoaderChange => "加载器将恢复为 {version}。",
        M::RestoreSafety => "会先为被替换的内容拍摄快照，并保留 30 天。",
        M::RestoreOtherGame => "此快照属于其他 Minecraft 版本或加载器，只能作为新实例恢复。",
        M::RestoreButton => "恢复",
        M::RestoringSnapshot => "正在恢复快照…",
        M::DeletingSnapshot => "正在删除快照…",
        M::ChooseWorld => "请至少选择一个世界。",
        M::ExportTitle => "导出 {name}",
        M::ExportNote => {
            "来自 Modrinth 和 CurseForge 的模组和资源包只会列出而不会复制：导入文件时会在作者允许的情况下重新下载。"
        }
        M::ExportLeftOut => "以下文件可能包含密码和登录信息，已排除：{files}。",
        M::ImportFrom => "{name} · Minecraft {version} · {loader}",
        M::ImportWorlds => "世界：{worlds}",
        M::ImportDownloads => "需要从 Modrinth 和 CurseForge 下载 {size}。",
        M::ImportNotDownloadable => "作者不允许下载，因此已排除：{files}。",
        M::ImportServerOnlyMods => "只在服务器上运行的模组，已略过：{names}",
        M::ImportServerPack => {
            "只有服务器玩家使用的文件来自它在 Modrinth 上的整合包，可能包含模组：它们是游戏启动时运行的程序。只从你信任的服务器下载世界。"
        }
        M::ImportServerPackCurseforge => {
            "服务器的整合包来自 CurseForge，目前还不能从服务器下载，因此只有其玩家使用的文件不会一起带来。"
        }
        M::ImportHostLeftOut => "主机没有把这些放进文件：{files}",
        M::ImportHostBlanked => "主机清空了这些密钥：{names}",
        M::ImportSpace => "需要 {size} 磁盘空间（含 1 GB 余量）。",
        M::ImportNeedsModrinth => "无法连接 Modrinth，无法下载文件中的模组。",
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
        M::ExportHintBoth => "导出为 Modrinth 的 .mrpack 文件，或 CurseForge 的 .zip 文件。",
        M::ExportButton => "导出…",
        M::ExportCurseForgeButton => "导出 CurseForge .zip…",
        M::ExportModrinthButton => "导出 Modrinth .mrpack…",
        M::Exporting => "正在导出…",
        M::Duplicate => "创建副本",
        M::Duplicating => "正在创建副本…",
        M::ChangeVersionOrLoader => "其他版本或加载器",
        M::ChangeVersionOrLoaderHint => "会创建一个副本，此实例保持不变。",
        M::CopyAs => "以其他版本或加载器复制",
        M::CopyAsButton => "换版本复制…",
        M::CopyAsKeywords => "更改 切换 更新 升级 降级 版本 加载器 Fabric Forge NeoForge Quilt",
        M::CopyAsTitle => "以其他版本或加载器复制",
        M::CopyFrom => "基于 {name}（{version}）新建一个实例，原实例保持不变。",
        M::CopyWhat => "要复制的内容",
        M::CopyWorlds => "世界",
        M::CopyNoWorlds => "此实例没有世界。",
        M::CopyWorldsOlder => "世界无法复制到较旧的 Minecraft 版本：旧版本无法打开它们。",
        M::CopyWorldsUnknown => "世界不会复制：Lodestone Launcher 无法判断 {version} 是否更新。",
        M::CopyWorldsUpgraded => {
            "Minecraft {version} 打开世界时会将其升级，且无法还原。原世界仍保留在 {name} 中。"
        }
        M::CopyWorldsModsGone => {
            "副本打开世界时，未一同复制的模组中的方块和物品会被移除。原世界仍保留在 {name} 中。"
        }
        M::CopyPacks => "资源包、光影和选项",
        M::CopyMods => "模组",
        M::CopyReading => "正在读取实例的模组…",
        M::CopyNoMods => "此实例没有模组。",
        M::CopyModsVanilla => "原版 Minecraft 不运行模组。",
        M::CopyModsOffline => "无法连接到 Modrinth，因此无法复制模组。",
        M::CopyModsFailed => "无法检查模组：{error}",
        M::CopyModsNoBuild => "没有适用于 {target} 的构建，以下模组不会复制：{names}。",
        M::CopyModsNotOnModrinth => "不在 Modrinth 上，以下模组不会复制：{names}。",
        M::CopyModsUnchecked => "无法在 Modrinth 上检查以下模组，因此不会复制：{names}。",
        M::CopyModsUncheckedCurseForge => {
            "无法在 CurseForge 上检查以下模组，因此不会复制：{names}。"
        }
        M::CopyOrUncheckMods => "或取消勾选“模组”，不带模组复制。",
        M::CopySameTarget => {
            "请选择其他 Minecraft 版本或加载器。若要原样复制此实例，请使用“维护”中的“创建副本”。"
        }
        M::CopyAsStart => "开始复制",
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
        M::SharedLead => "按键、视频、声音和其他游戏内选项，在每个共享它们的实例中保持一致。",
        M::UseSharedSettings => "使用共享设置",
        M::SharingFrom => "接收来自 {name} 的游戏内更改。",
        M::SharingMain => "此实例中游戏内的更改会同步到所有实例。",
        M::SharingNoMain => "没有主实例：共享值只能在设置中更改。",
        M::MainInstance => "主实例",
        M::MainInstanceHint => "它的游戏内更改会在其他共享设置的实例下次启动时同步过去。",
        M::IsMainInstance => "这是你的主实例",
        M::MakeMain => "设为主实例",
        M::StopGameToChange => "请先停止游戏再更改：游戏退出时会重写 options.txt。",
        M::NotShared => "未共享（启动器设置）",
        M::SharedKey => "共享",
        M::OverriddenKey => "已覆盖",
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
        M::AddModpackHint => "实例已有的内容都不会被替换。",
        M::AddModpackServerHint => "用于客户端模组，例如优化模组。{server} 的模组保持不变。",
        M::AddModpackEllipsis => "添加整合包…",
        M::RemoveAddedModpackNote => "移除该整合包添加的文件。你修改过的文件会保留。",
        M::SnapshotsNotChanged => "无法更改快照",
        M::RestoreKeptAsideTitle => "有些文件妨碍了恢复",
        M::SnapshotRestored => "已恢复“{name}”。",
        M::ImportLeftOutTitle => "部分文件未导入",
        M::ImportLeftOut => "这些文件无法下载，因此新实例中没有它们：{files}。",
        M::ImportPackSkippedTitle => "缺少整合包的文件",
        M::ImportPackSkipped => {
            "无法下载或放置服务器的整合包（没有网络连接、可用空间不足，或有文件未通过校验），因此新实例中没有只有其玩家使用的文件。"
        }
        M::ImportPackSkippedGone => {
            "服务器的整合包已不在 Modrinth 上，因此新实例中没有只有其玩家使用的文件。请让服务器所有者更新整合包。"
        }
        M::ImportPackSkippedCurseforge => {
            "服务器基于 CurseForge 整合包，目前还无法从服务器安装，因此新实例中没有只有其玩家使用的文件。"
        }
        M::RestoreKeptAside => "恢复期间写入的文件保存在 {folder}。",
        M::ImportNoRoom => "磁盘空间不足：需要 {size}（含 1 GB 余量），可用 {free}。",
        M::ImportSpaceFree => "需要 {size} 磁盘空间（含 1 GB 余量）；可用 {free}。",
        M::ExportNotDownloadable => "这些无法下载，因此导入该文件的人不会得到它们：{files}。",
        M::DeleteUnreadableNote => "它们将被永久删除。",
        M::SnapshotsGroup => "已保存的快照",
        M::BackUpWorldTitle => "备份世界",
        M::RestoreAsNewWorld => "恢复为新世界",
        M::WorldBackupWorld => "世界",
        M::WorldBackupDefaultName => "世界备份",
        M::WorldBackupBeforeRestoring => "恢复“{name}”之前",
        M::BackUpButton => "备份",
        M::BackingUpWorld => "正在备份 {name}…",
        M::WorldBackedUp => "已备份 {name}。",
        M::NoWorldsToBackUp => "此实例还没有世界。",
        M::RestoreWorldTitle => "恢复 {world}",
        M::RestoreWorldBody => {
            "{world} 将回到“{name}”（{when}）时的状态。会先把世界现在的样子另存为一个世界备份。"
        }
        M::RestoreWorldGoneBody => "{world} 已不在此实例中。它会以“{name}”（{when}）时的状态回来。",
        M::SaveAndRestore => "保存并恢复",
        M::SavingWorld => "正在保存世界现在的样子…",
        M::UnpackingWorldBackup => "正在解压世界备份…",
        M::WorldRestored => "已恢复 {world}。",
        M::WorldRestoredSafety => "原来的世界保存在世界备份“{name}”中。",
        M::WorldRestoredAsNew => "已恢复为新世界 {folder}。",
        M::DeleteWorldBackupTitle => "删除这个世界备份？",
        M::DeleteWorldBackupBody => "“{name}”会移到{trash}。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShareNotShared => ["有 {count} 个文件不会被分享"; 2],
        P::ShareAddedCount => ["添加 {count} 项"; 2],
        P::ShareRemovedCount => ["移除 {count} 项"; 2],
        P::ShareChangedCount => ["更改 {count} 项"; 2],
        P::UnreadableImages => ["{count} 个镜像无法读取"; 2],
        P::RestoreMissing => ["镜像存储丢失了其中 {count} 个文件，无法完整恢复。"; 2],
        P::ImportModsWarning => {
            ["包含 {count} 个模组：它们是游戏启动时运行的程序。只导入你信任的人发送的文件。"; 2]
        }
        P::ShareToggledCount => ["启用或禁用 {count} 项"; 2],
        P::CodePackChanges => ["+ {count} 项更改"; 2],
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
        P::CopyWorldsCount => ["{count} 个世界"; 2],
        P::CopyCheckingMods => ["正在 Modrinth 上检查 {count} 个模组…"; 2],
        P::CopyModsFound => ["{count} 个模组中有 {found} 个有适用于 {target} 的构建。"; 2],
        P::RestorePutsBack => ["恢复 {count} 个文件：{names}"; 2],
        P::RestoreRemoves => ["删除 {count} 个文件：{names}"; 2],
        P::RestoreChanges => ["更改 {count} 个文件：{names}"; 2],
        P::ImportCarriedMods => {
            ["其中 {count} 个包含在文件本身中，而不是从 Modrinth 或 CurseForge 下载的：{names}"; 2]
        }
        P::ImportFromServer => [
            "来自 {server} 的服务器备份：包括它的世界和玩家会获得的模组。{count} 个服务器文件不会被带入（服务器的设置、管理员和插件）。",
            "来自 {server} 的服务器备份：包括它的世界和玩家会获得的模组。{count} 个服务器文件不会被带入（服务器的设置、管理员和插件）。",
        ],
        P::WorldBackupLeftOut => ["{count} 个文件未包含"; 2],
    }
}
