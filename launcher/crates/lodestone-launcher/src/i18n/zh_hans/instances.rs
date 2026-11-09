use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::TwoSentences => "{first}{second}",
        M::ShareCodeReady => "{name} 已就绪",
        M::ShareCodeNotInstalled => "未安装：{names}。",
        M::ShareCodeContentFailed => "无法安装分享码中的内容",
        M::ShareCodeOffline => "无法连接 Modrinth，因此创建的实例不含其内容。",
        M::TitleAndMessage => "{title}：{message}",
        M::Vanilla => "原版",

        M::StagePreparing => "正在准备",
        M::StageJava => "正在下载 Java",
        M::StageLibraries => "正在下载依赖库",
        M::StageLoader => "正在安装模组加载器",
        M::StageProcessors => "正在为模组加载器修补 Minecraft",
        M::StageAssets => "正在下载游戏资源",
        M::StageNatives => "正在解压本地库",
        M::StageStarting => "正在启动 Java",
        M::ProgressDetail => "{files_done}/{files_total} · {done} MB/{total} MB",

        M::PreparingNote => "正在准备 Minecraft {version}（{loader}）",
        M::JoiningNote => "游戏启动后将加入 {address}",
        M::VerifyingNote => "正在校验 Minecraft {version}（{loader}）的游戏文件",
        M::RosettaNote => "此版本通过 Rosetta 2 在 Intel（x86_64）Java 中运行。",
        M::StartingJavaNote => "正在启动 Java：{command}",
        M::StoppedNote => "已停止",
        M::ExitedWithCode => "Minecraft 已退出，退出代码为 {code}",
        M::EndedBySignal => "Minecraft 被信号终止",
        M::CancellingNote => "正在取消…",
        M::StoppingNote => "正在停止 Minecraft",
        M::Cancelled => "已取消",

        M::CouldntSignIn => "无法登录",
        M::RosettaNeeded => "需要 Rosetta 2",
        M::RosettaMissing => {
            "此 Minecraft 版本需要 Intel（x86_64）Java，它在 Apple Silicon 上通过 Rosetta 2 运行。Rosetta 尚未安装。请在“终端”中运行以下命令安装，然后再次点击“开始游戏”：\n\nsoftwareupdate --install-rosetta"
        }
        M::CouldntStartMinecraft => "无法启动 Minecraft",
        M::CouldntStartJava => "无法启动 Java",
        M::CouldntVerify => "无法校验游戏文件",
        M::GameFilesVerified => "游戏文件已校验",
        M::EveryFileIntact => "所有文件均完好。{summary}",
        M::ModpackUpdating => "整合包正在更新",
        M::WaitForUpdate => "请等待更新完成，然后点击“开始游戏”。",
        M::StopBeforeDeleting => "请先停止游戏，再删除该实例。",
        M::FilesBusy => "请等待该实例的文件处理完成，然后重试。",
        M::CouldntSavePlayed => "无法保存上次游玩时间：{error}",
        M::CouldntSavePlayTime => "无法保存游戏时长：{error}",
        M::ModpackExported => "整合包已导出",
        M::CouldntExportModpack => "无法导出整合包",
        M::CouldntSwitchLoader => "无法切换到模组加载器",
        M::NoModLoaderFor => {
            "Lodestone Launcher 支持的模组加载器都无法在 Minecraft {version} 上运行：Fabric 从 1.14 起才支持，而 Forge 没有适用于该版本的构建。"
        }
        M::SwitchedTo => "已切换到 {loader}",
        M::SwitchedToMessage => {
            "{name} 现在使用 {loader} {version}。下次点击“开始游戏”时会自动安装。"
        }

        M::SearchLog => "搜索日志",
        M::LevelDebug => "调试",
        M::LevelInfo => "信息",
        M::LevelWarn => "警告",
        M::LevelError => "错误",
        M::LogEmpty => "点击“开始游戏”后，游戏输出会显示在这里。",
        M::NoLinesMatch => "没有符合筛选条件的行。",
        M::JumpToLatest => "跳到最新",
        M::CopyLog => "复制日志",

        M::ImportTitle => "从其他启动器导入",
        M::ImportFromHere => "从此处导入",
        M::LinksLeftOut => "指向游戏文件夹以外的符号链接已跳过。",
        M::NewestRelease => "最新正式版",
        M::NewestSnapshot => "最新快照版",
        M::NoInstances => "没有实例。",
        M::LookingForLaunchers => "正在查找其他启动器",
        M::NoLaunchersFound => {
            "在常见位置未找到其他启动器。请选择某个启动器或实例的文件夹以从中导入。"
        }
        M::Waiting => "等待中",
        M::FindingVersion => "正在确定版本和加载器",
        M::Imported => "已导入",
        M::ImportedNote => "已导入。{note}",
        M::Copying => "正在复制 {done} / {total}",
        M::MatchingModrinth => "正在将模组和资源包与 Modrinth 匹配",
        M::CheckToImport => "勾选要导入的实例。其文件会被复制，原启动器中的文件保持不变。",
        M::LeaveWorldsOut => "不导入世界",
        M::LeaveWorldsOutSize => "不导入世界（{size}）",
        M::Import => "导入",
        M::ChooseFolder => "选择文件夹…",
        M::StopImportingTitle => "停止导入？",
        M::StopImportingBody => "尚未完成的导入会停止，已复制的内容会被删除。已导入的实例会保留。",
        M::StopImporting => "停止导入",
        M::KeepImporting => "继续导入",

        M::NotAModpack => "不是整合包",
        M::NotMrpack => {
            "{name} 不是整合包。Modrinth 整合包的扩展名为 .mrpack，CurseForge 整合包是包含 manifest.json 的 .zip 文件。"
        }
        M::CurseForgeUnreachable => {
            "这是 CurseForge 整合包，而此 Lodestone Launcher 无法连接 CurseForge。"
        }
        M::CurseForgePackOnWebsite => {
            "此整合包只能从 CurseForge 网站下载。请在网站上下载，然后将 .zip 拖到 Lodestone Launcher 上。"
        }
        M::Modpack => "整合包",
        M::ReadingModpack => "正在读取整合包",
        M::ModpackReady => "{name} 已就绪",
        M::ModpackReadyMessage => {
            "Minecraft {version}，使用 {loader}。点击“开始游戏”即可下载并启动游戏。"
        }
        M::CouldntImportModpack => "无法导入整合包",
        M::CouldntUpdateModpack => "无法更新整合包",
        M::StopGameFirst => "请先停止游戏。",
        M::ModpackUpdated => "整合包已更新",
        M::ModpackUpdatedMessage => {
            "{name} 现在运行 {pack} {version}。被替换的文件已保存在 {path}。"
        }
        M::UpdatedButNotSaved => "文件已更新，但无法保存实例：{error}",
        M::DownloadingName => "正在下载 {name}",
        M::InstallingName => "正在安装 {name}",
        M::CopyingConfigs => "正在复制整合包的配置文件",
        M::RecordingFiles => "正在记录整合包的文件",
        M::SavingSnapshot => "正在保存实例文件的快照",
        M::BackingUp => "正在备份 {name}",
        M::UpdatingTo => "正在更新到 {version}",

        M::SearchModpacks => "在 Modrinth 上搜索整合包，或粘贴 Modrinth 链接",
        M::SearchModpacksCurseForge => "在 CurseForge 上搜索整合包",
        M::SortRelevance => "相关性",
        M::SortDownloads => "下载量",
        M::SortFollows => "关注数",
        M::SortUpdated => "最近更新",
        M::LinkNotModpack => "该链接指向的是{kind}，不是整合包。请在实例的“内容”标签页中添加。",
        M::KindMod => "模组",
        M::KindResourcePack => "资源包",
        M::KindShaderPack => "光影包",
        M::KindDataPack => "数据包",
        M::KindPlugin => "插件",
        M::SearchingModrinth => "正在搜索 Modrinth",
        M::NoModpacksMatch => "没有匹配的整合包。",
        M::ByAuthor => "作者 {author} · {downloads} 次下载",
        M::PickModpack => "选择一个整合包以查看其版本。",
        M::LoadingModpack => "正在加载整合包",
        M::Versions => "版本",
        M::Description => "简介",
        M::NoVersions => "此整合包没有任何版本。",
        M::LoadingVersions => "正在加载版本",
        M::FindModButton => "帮我找出来",
        M::FinderTitle => "找出导致“{name}”崩溃的模组",
        M::FinderIntro => {
            "启动器会多次启动游戏，每次禁用可能导致崩溃的模组中的一半，直到只剩下一个。完成后，你的模组会恢复原样。"
        }
        M::FinderAutomatic => {
            "游戏在启动时崩溃，所以你什么都不用做。每次尝试会在游戏崩溃或进入主菜单时结束。"
        }
        M::FinderManual => {
            "游戏是在你游玩时崩溃的，所以每次尝试都需要你来玩。重复之前导致崩溃的操作。如果再次崩溃，下一次尝试会自动开始；如果没有，请按“没有崩溃”。"
        }
        M::FinderStart => "开始",
        M::FinderReading => "正在检查你的模组",
        M::FinderTry => "第 {number} 次尝试",
        M::FinderStarting => "正在启动游戏",
        M::FinderWaiting => "正在等待，看游戏是否崩溃",
        M::FinderSettling => "已进入主菜单，正在确认游戏不会关闭",
        M::FinderPlay => "一直玩到崩溃为止。如果没有崩溃，请按“没有崩溃”。",
        M::FinderAsk => "游戏已关闭。问题出现了吗？",
        M::FinderDidntCrash => "没有崩溃",
        M::FinderItCrashed => "崩溃了",
        M::FinderStopping => "正在停止游戏",
        M::FinderFound => "找到了",
        M::FinderFoundOne => "禁用 {name} 后，游戏不会崩溃。",
        M::FinderFoundGroup => {
            "禁用这些模组后，游戏不会崩溃：{names}。它们相互依赖，只能一起禁用。"
        }
        M::FinderAlsoOff => "依赖它的模组也会保持禁用：{names}。",
        M::FinderKeepOff => "保持禁用",
        M::FinderPutBack => "全部重新启用",
        M::FinderNotFound => {
            "即使禁用了这些模组，游戏仍然崩溃，所以原因可能不是模组，或者崩溃只是偶尔发生。你的模组已恢复原样。"
        }
        M::FinderRefused => {
            "有些模组离开被禁用的模组就无法加载，所以查找只能停止。你的模组已恢复原样。"
        }
        M::FinderFailed => "出了点问题，查找已停止。你的模组已恢复原样。",
        M::FinderStopped => "已停止查找。你的模组已恢复原样。",
        M::FinderStop => "停止",
        M::FinderInterruptedTitle => "要完成在“{name}”中查找模组吗？",
        M::FinderInterrupted => "启动器在查找中途关闭了，所以“{name}”的部分模组仍处于禁用状态。",
        M::FinderPutBackFailed => "部分模组无法重新启用：{error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::CheckSummary => {
            ["已在 {ms} 毫秒内检查 {count} 个游戏文件：{unchanged} 个自上次检查以来未变，{hashed} 个已计算哈希，{downloaded} 个已下载。";
                2]
        }
        P::FilesRepaired => ["已重新下载 {count} 个缺失或损坏的文件。{summary}"; 2],
        P::BrokenFolders => ["有 {count} 个实例文件夹无法读取，例如 {path}。"; 2],
        P::FilesSaved => ["已将 {count} 个文件保存到 {path}。"; 2],
        P::CurseForgeLeftOut => {
            ["有 {count} 个文件仅在 CurseForge 上，CurseForge 的文件不能放进 Modrinth 整合包，因此已跳过：{names}。";
                2]
        }
        P::FilesLeftOut => ["有 {count} 个文件被跳过，例如 {name}。"; 2],
        P::LinesShown => ["显示 {shown} 行，共 {count} 行"; 2],
        P::LinesShownFollowing => ["显示 {shown} 行，共 {count} 行 · 自动滚动"; 2],
        P::ProblemAndMore => ["{problem}（另有 {count} 个问题）"; 2],
        P::InstancesToCopy => ["将复制 {count} 个实例，共 {size}。"; 2],
        P::ImportInstances => ["导入 {count} 个实例"; 2],
        P::ShareCodeInstalled => ["已从分享码安装 {count} 项内容。"; 2],
        P::PackFilesToDownload => {
            ["有 {count} 个文件需要从 CurseForge 网站下载：实例的“内容”标签页会列出它们。"; 2]
        }
        P::PackFilesLeftOut => ["整合包中有 {count} 个文件被跳过，例如 {path}。"; 2],
        P::WorldsBackedUp => ["已先备份 {count} 个世界。"; 2],
        P::DownloadingFiles => ["正在下载 {count} 个文件"; 2],
        P::FinderModsLeft => ["还有 {count} 个模组待检查"; 2],
        P::FinderModsOff => ["已禁用 {count} 个模组"; 2],
        P::FinderTriesLeft => ["大约还需 {count} 次尝试"; 2],
    }
}
