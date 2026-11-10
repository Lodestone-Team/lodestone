use crate::i18n::{ContentMsg as M, ContentPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KindMods => "模组",
        M::KindResourcePacks => "资源包",
        M::KindShaders => "光影",
        M::Vanilla => "原版",
        M::SearchInstalledOrModrinth => "搜索已安装内容，或在 Modrinth 上查找更多",
        M::InstalledCount => "已安装 · {count}",
        M::EnabledCount => "已启用 · {count} · 重叠时以最上面的包为准",
        M::AvailableCount => "可用 · {count}",
        M::OnModrinthCount => "Modrinth 上 · {count}",
        M::OnModrinth => "Modrinth 上",
        M::OnCurseForgeCount => "CurseForge 上 · {count}",
        M::OnCurseForge => "CurseForge 上",
        M::NoInstalledMatch => "已安装 · {count} · 没有匹配的已安装{kind}。",
        M::OnlyTarget => "仅限 {target}",

        M::StopGameFirst => "请先停止游戏：游戏退出时会重写这些文件。",
        M::WaitForInstall => "请等待安装完成。",
        M::GameRunning => {
            "游戏正在运行。停止游戏后才能更改其内容：游戏退出时会重写 options.txt 和光影设置。"
        }

        M::AllModsUpToDate => "来自 Modrinth 的模组都已是最新版本。",
        M::AllResourcePacksUpToDate => "来自 Modrinth 的资源包都已是最新版本。",
        M::AllShaderPacksUpToDate => "来自 Modrinth 的光影包都已是最新版本。",
        M::CouldntCheckUpdates => "无法检查更新：{error}",

        M::CheckingProject => "正在检查 {name}",
        M::CheckingDependencies => "正在检查依赖项",
        M::FilesProgress => "{done}/{total} 个文件",
        M::BytesProgress => "{done} / {total}",

        M::NothingInstalled => "没有安装任何内容。",
        M::InstalledOne => "已安装 {name}。",
        M::InstalledMany => "已安装 {names} 和 {last}。",
        M::Replaced => "已替换 {files}。",
        M::Skipped => "已跳过 {name}：{reason}。",

        M::AddedFiles => "已添加 {files}。",
        M::FileError => "{name}：{error}",
        M::NotAddedFiles => "未添加：{files}",
        M::AddedAndNotAdded => "已添加 {added}。未添加：{failed}",
        M::AddToMods => "添加到模组",
        M::AddToResourcePacks => "添加到资源包",
        M::AddToShaders => "添加到光影",

        M::LinkNotInstallable => "该链接指向“{kind}”类型的项目，Lodestone Launcher 暂不支持安装。",

        M::CheckForUpdates => "检查更新",
        M::UpdateAll => "全部更新（{count}）",
        M::AddFile => "添加文件…",
        M::AddFolder => "添加文件夹…",
        M::ItemFolder => "{name} · 文件夹",
        M::Compatible => "兼容",
        M::OlderVersion => "旧版本",
        M::NewerVersion => "新版本",
        M::NotOnModrinth => "不在 Modrinth 上",
        M::UpdateTo => "更新到 {version}",
        M::NoModsYet => "还没有模组。浏览 Modrinth，或将 .jar 文件拖放到这里。",
        M::NoResourcePacksYet => "还没有资源包。浏览 Modrinth，或将 .zip 文件或文件夹拖放到这里。",
        M::NoShaderPacksYet => "还没有光影包。浏览 Modrinth，或将 .zip 文件或文件夹拖放到这里。",
        M::ReadingFolder => "正在读取 {folder} 文件夹",
        M::CouldntReadFolder => "无法读取 {folder} 文件夹：{error}",
        M::SomeFilesNotMatched => "部分文件无法与 Modrinth 匹配：{error}",
        M::ShadersOff => "关闭光影",
        M::NormalLook => "Minecraft 的原始画面。",

        M::ModsNeedLoader => "模组需要模组加载器",
        M::ShadersNeedLoader => "光影需要模组加载器",
        M::VanillaSwitchMods => {
            "此实例是原版 Minecraft。模组需要通过 {loader} 等模组加载器运行；切换会保留你的世界和设置，{loader} 会在你下次点击“开始游戏”时安装。"
        }
        M::VanillaSwitchShaders => {
            "此实例是原版 Minecraft。光影需要通过 {loader} 等模组加载器运行；切换会保留你的世界和设置，{loader} 会在你下次点击“开始游戏”时安装。"
        }
        M::VanillaNoLoaderMods => {
            "此实例是原版 Minecraft {version}。模组需要模组加载器，但 Lodestone Launcher 支持的加载器都无法在此版本上运行：Fabric 从 1.14 开始支持，Forge 也没有适用于此版本的构建。新建一个较新版本的实例即可使用模组。"
        }
        M::VanillaNoLoaderShaders => {
            "此实例是原版 Minecraft {version}。光影需要模组加载器，但 Lodestone Launcher 支持的加载器都无法在此版本上运行：Fabric 从 1.14 开始支持，Forge 也没有适用于此版本的构建。新建一个较新版本的实例即可使用光影。"
        }
        M::VanillaCheckFailedMods => {
            "此实例是原版 Minecraft。模组需要模组加载器，但无法检查哪个加载器适用：{error}"
        }
        M::VanillaCheckFailedShaders => {
            "此实例是原版 Minecraft。光影需要模组加载器，但无法检查哪个加载器适用：{error}"
        }
        M::VanillaCheckingMods => {
            "此实例是原版 Minecraft。模组需要通过模组加载器运行；正在检查哪个加载器能在 Minecraft {version} 上运行…"
        }
        M::VanillaCheckingShaders => {
            "此实例是原版 Minecraft。光影需要通过模组加载器运行；正在检查哪个加载器能在 Minecraft {version} 上运行…"
        }
        M::SwitchTo => "切换到 {loader}",
        M::NewFabricInstance => "新建 Fabric 实例…",

        M::ShaderModUsingPack => "{shader_mod} 正在使用 {pack}。",
        M::ShaderModUsingBuiltIn => "{shader_mod} 正在使用内置光影。",
        M::ShaderModInstalledOff => "{shader_mod} 已安装；光影已关闭。",
        M::ShaderModPickPack => {
            "在下方选择一个光影包。{shader_mod} 会在游戏启动时从 {file} 读取你的选择。"
        }
        M::ShadersNeedShaderMod => "光影需要光影模组",
        M::OptiFineNote => {
            "OptiFine 也可以使用，但它不在 Modrinth 上，且其许可协议不允许 Lodestone 下载。如果你更喜欢它，请从 optifine.net 下载，然后在“{mods}”中用“{add_file}”添加 jar 文件（Fabric 还需要 OptiFabric）。"
        }
        M::LookingForShaderMod => "正在 Modrinth 上查找适用于此版本的光影模组",
        M::CouldntAskModrinth => "无法向 Modrinth 查询：{error}",
        M::InstallShaderMod => "安装 {shader_mod} {version}",
        M::ShaderModRunsOn => {
            "Minecraft 本身无法加载光影包，需要借助光影模组。{shader_mod} 可在此实例的 {loader} 上运行。"
        }
        M::ShaderModComesWith => "它会附带安装 {shader_mod} 所需的 {dependency}。",
        M::ShaderModUnavailable => {
            "Minecraft 本身无法加载光影包，需要借助光影模组。{reason} Fabric 实例始终可以使用 Iris。"
        }

        M::BrowsingNeedsNetwork => "浏览 Modrinth 需要网络连接。",
        M::SortRelevance => "相关性",
        M::SortDownloads => "下载量",
        M::SortFollows => "关注数",
        M::SortNewest => "最新",
        M::SortUpdated => "最近更新",
        M::SearchingModrinth => "正在搜索 Modrinth",
        M::NothingMatches => "Modrinth 上没有匹配的结果。",
        M::Filters => "筛选",
        M::FiltersCount => "筛选 · {count}",
        M::FilterCategories => "分类",
        M::FilterFeatures => "功能",
        M::FilterResolution => "分辨率",
        M::FilterPerformance => "性能影响",
        M::FilterEnvironment => "安装位置",
        M::EnvAny => "不限",
        M::EnvClientOnly => "仅客户端",
        M::EnvServerToo => "服务器也需安装",
        M::FilterEnvironmentHint => {
            "仅客户端的模组可在任何服务器上使用；其他模组也必须安装在服务器上。"
        }
        M::FilterOther => "其他",
        M::FilterOpenSource => "仅开源",
        M::ClearFilters => "清除筛选",
        M::FiltersModrinthOnly => "这些筛选仅适用于 Modrinth 的结果。",
        M::CatAdventure => "冒险",
        M::CatCursed => "诅咒",
        M::CatDecoration => "装饰",
        M::CatEconomy => "经济",
        M::CatEquipment => "装备",
        M::CatFood => "食物",
        M::CatGameMechanics => "游戏机制",
        M::CatLibrary => "前置库",
        M::CatMagic => "魔法",
        M::CatManagement => "管理",
        M::CatMinigame => "小游戏",
        M::CatMobs => "生物",
        M::CatOptimization => "优化",
        M::CatSocial => "社交",
        M::CatStorage => "存储",
        M::CatTechnology => "科技",
        M::CatTransportation => "交通",
        M::CatUtility => "实用",
        M::CatWorldGen => "世界生成",
        M::CatCombat => "战斗",
        M::CatModded => "模组",
        M::CatRealistic => "写实",
        M::CatSimplistic => "简约",
        M::CatThemed => "主题",
        M::CatTweaks => "微调",
        M::CatVanillaLike => "原版风格",
        M::CatAudio => "音频",
        M::CatBlocks => "方块",
        M::CatCoreShaders => "核心着色器",
        M::CatEntities => "实体",
        M::CatEnvironment => "环境",
        M::CatFonts => "字体",
        M::CatGui => "界面",
        M::CatItems => "物品",
        M::CatLocale => "语言",
        M::CatModels => "模型",
        M::CatResolutionLow => "8x 及以下",
        M::CatResolutionHigh => "512x 及以上",
        M::CatCartoon => "卡通",
        M::CatFantasy => "奇幻",
        M::CatSemiRealistic => "半写实",
        M::CatAtmosphere => "大气",
        M::CatBloom => "泛光",
        M::CatColoredLighting => "彩色光照",
        M::CatFoliage => "植被",
        M::CatPathTracing => "路径追踪",
        M::CatPbr => "PBR",
        M::CatReflections => "反射",
        M::CatShadows => "阴影",
        M::CatPotato => "土豆",
        M::CatLow => "低",
        M::CatMedium => "中",
        M::CatHigh => "高",
        M::CatScreenshot => "截图",
        M::NothingOnCurseForge => "CurseForge 上没有匹配的结果。",
        M::PageOf => "第 {page} 页，共 {pages} 页",
        M::ByAuthor => "作者：{author}",
        M::InstalledBadge => "已安装",
        M::OptionalOnServers => "服务器上可选",
        M::DownloadsCompact => "{count}次下载",
        M::FollowsCompact => "{count}人关注",
        M::FollowersCompact => "{count}位关注者",

        M::LoadingProject => "正在加载项目",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::Versions => "版本",
        M::Description => "简介",
        M::LoadingVersions => "正在加载版本",
        M::ShowAllVersions => "显示适用于其他加载器和 Minecraft 版本的版本",
        M::AlsoInstallDependencies => "同时安装必需的依赖项",
        M::NoVersionFits => "{project} 没有适用于此实例的版本。打开“{show_all}”即可查看全部版本。",
        M::ShowVersionsShort => "显示版本…",
        M::InstallAnyway => "仍然安装…",
        M::InstallItAnyway => "确认仍然安装",
        M::InstallWith => "与 {shader_mod} 一起安装",
        M::Release => "正式版",
        M::Beta => "测试版",
        M::Alpha => "早期测试版",

        M::ModrinthTab => "Modrinth",
        M::CurseForgeTab => "CurseForge",
        M::SearchingCurseForge => "正在搜索 CurseForge",
        M::ViewOnCurseForge => "在 CurseForge 上查看",
        M::NotOnEitherPlatform => "不在 Modrinth 或 CurseForge 上",
        M::SortPopular => "热门",
        M::SortName => "名称",
        M::WebsiteDownload => "仅限网站下载",
        M::GetOnCurseForge => "在 CurseForge 上获取",
        M::BlockedTitle => "请从 CurseForge 下载这些文件",
        M::BlockedLead => {
            "这些文件的作者不允许启动器下载。请在浏览器中逐个下载，文件一出现在 {folder} 中，Lodestone 就会添加它。"
        }
        M::BlockedNoFolder => {
            "这些文件的作者不允许启动器下载。请在浏览器中逐个下载，然后选择保存它们的文件夹。"
        }
        M::BlockedOpen => "打开下载页面",
        M::BlockedOpenAll => "打开所有页面",
        M::BlockedAdded => "已添加",
        M::BlockedWaiting => "正在等待 {file}",
        M::BlockedChangeFolder => "监视其他文件夹…",
        M::BlockedSkip => "跳过其余文件",
        M::WatchFolderPrompt => "监视此文件夹",
        M::CouldntAddDownload => "无法添加 {name}：{error}",
        M::AddedDownloads => "已添加 {names}。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Downloads => ["{count} 次下载"; 2],
        P::Follows => ["{count} 人关注"; 2],
        P::Followers => ["{count} 位关注者"; 2],
        P::MoreVersions => ["Modrinth 上还有 {count} 个版本。"; 2],
    }
}
