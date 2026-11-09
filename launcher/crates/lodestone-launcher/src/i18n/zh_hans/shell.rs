use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "退出 {app}",
        M::Language => "语言",
        M::LanguageSystem => "系统",
        M::LanguageSystemIs => "系统（{language}）",
        M::Zoom => "缩放",
        M::ViewMenu => "显示",
        M::ZoomIn => "放大",
        M::ZoomOut => "缩小",
        M::ActualSize => "实际大小",
        M::EditMenu => "编辑",
        M::Undo => "撤销",
        M::Redo => "重做",
        M::Undid => "已撤销：{change}",
        M::Redid => "已重做：{change}",
        M::ChangeTo => "{setting} → {value}",
        M::SettingsOf => "{name} 的设置",

        M::Play => "开始游戏",
        M::Playing => "游戏中",
        M::TabContent => "内容",
        M::TabWorlds => "世界",
        M::Settings => "设置",
        M::TabLogs => "日志",

        M::LastPlayed => "最近游玩",
        M::Instances => "实例",
        M::NoInstancesYet => "还没有实例。",
        M::NewInstance => "新建实例…",
        M::ImportInstances => "导入实例…",
        M::Servers => "服务器",
        M::AppVersion => "Lodestone Launcher v{version}",
        M::PlayedInTotal => "{when} · 共 {time}",
        M::ImportProgress => "{percent} · {label}",
        M::GroupModded => "{loader}（模组）",
        M::Vanilla => "原版",
        M::GroupImported => "已导入",

        M::NoInstanceSelected => "未选择实例",
        M::NoInstanceSelectedHint => "创建一个实例，即可选择 Minecraft 版本和加载器。",
        M::MinecraftVersion => "Minecraft {version}",
        M::MinecraftWith => "Minecraft {version}，{loader}",
        M::HeroPlayed => "{game}。已游玩 {time}。",
        M::HeroNotPlayed => "{game}。点击“开始游戏”即可下载并启动游戏。",
        M::CreateFirstInstance => "创建你的第一个实例",
        M::InstanceExplainer => "实例是一个 Minecraft 版本，拥有自己的模组、世界和设置。",
        M::ContentPlaceholder => "有了实例后，内容会显示在这里。",
        M::WorldsPlaceholder => "有了实例后，世界会显示在这里。",
        M::SettingsPlaceholder => "有了实例后，设置会显示在这里。",
        M::LogsPlaceholder => "有了实例后，日志会显示在这里。",
        M::LinkedTo => "已关联到 {core} 上的 {server}",
        M::LinkedToRemoved => "已关联到 {core} 上的 {server}（该核心已从“服务器”页面移除）",
        M::PickerInstance => "实例",
        M::PickerVersion => "版本",
        M::NoInstances => "没有实例",
        M::AndJoin => "并加入 {server}",
        M::PlayWithoutJoining => "开始游戏但不加入",

        M::DeleteNamed => "删除“{name}”？",
        M::DeleteWorldBody => "该世界的文件夹会移到{trash}，你可以从那里恢复。它的备份会保留。",
        M::DeleteInstanceBody => {
            "该实例的文件夹（包括其中的世界、模组和设置）会移到{trash}。你可以从那里恢复。"
        }
        M::ReplaceWorldTitle => "用此备份替换“{name}”？",
        M::ReplaceWorldBody => {
            "“{name}”会恢复到{when} 时的状态。此后的所有改动都会从世界中丢失，但会先备份，当前文件夹会移到{trash}。"
        }
        M::BackUpAndReplace => "备份并替换",
        M::SwitchToLoader => "切换到 {loader}？",
        M::SwitchToVanilla => "切换到原版？",
        M::ModsWontLoadOn => "以下已启用的模组是为其他加载器制作的，在 {loader} 上无法加载：",
        M::ModsWontLoadVanilla => "原版 Minecraft 不加载任何模组，因此以下已启用的模组无法加载：",
        M::ModMadeFor => "{loader} 模组",
        M::TurningOffRenames => {
            "禁用后，它们会被重命名为 .disabled，你之后可以在“内容”中重新启用。"
        }
        M::WorldsAtRisk => "使用过这些模组中的方块或物品的世界可能会丢失它们。请先备份你的世界。",
        M::TurnOffAndSave => "禁用并保存",
        M::SaveAndKeep => "保存并保留",
        M::RemoveAccountTitle => "移除“{name}”？",
        M::RemoveAccountBody => {
            "Lodestone Launcher 会忘记此账户，并从你的{keychain}中删除其保存的登录信息。你之后可以重新添加。"
        }
        M::RemoveAccount => "移除账户",
        M::MinecraftCrashed => "Minecraft 崩溃了：{description}。",
        M::MinecraftExitCode => "Minecraft 已停止，退出代码为 {code}。",
        M::MinecraftStopped => "Minecraft 意外停止。",
        M::CrashTitle => "“{name}”崩溃了",
        M::LikelyCause => "可能的原因",
        M::OpenCrashReport => "打开崩溃报告",
        M::CauseWrongJava => {
            "Java 版本不对。此 Minecraft 版本（或某个模组）需要其他版本的 Java。请在“设置”中清除自定义 Java，以使用 Mojang 提供的 Java。"
        }
        M::CauseOutOfMemory => "内存不足。请在“设置”中提高最大内存，或关闭其他应用。",
        M::CauseNatives => {
            "本地库缺失，或与处理器架构不符。在 Apple 芯片上，1.19 之前的版本会通过 Rosetta 2 在 Intel 版 Java 中运行；请确认已安装 Rosetta，并且自定义 Java 不是 arm64 版。"
        }
        M::CauseJvmOptions => "Java 拒绝了其中一项设置。请检查“设置”中的 JVM 参数和内存。",
        M::CauseGraphics => {
            if cfg!(target_os = "macos") {
                "游戏无法创建窗口或初始化 OpenGL。请更新 macOS，或尝试不使用自定义窗口大小或全屏。"
            } else {
                "游戏无法创建窗口或初始化 OpenGL。请更新显卡驱动，或尝试不使用自定义窗口大小或全屏。"
            }
        }
        M::CopyLog => "复制日志",

        M::UpdateTitle => "更新“{name}”？",
        M::ModpackSummary => "{pack} {from} → {to}。Minecraft {game}，{loader}。需下载 {size}。",
        M::NoFilesChange => "没有文件变化，只会记录新版本。",
        M::UpdatedFiles => "已更新（{count}）",
        M::AddedFiles => "已添加（{count}）",
        M::RemovedFiles => "已移除（{count}）",
        M::ReplacedOwnFiles => "原本属于你、现由整合包提供的文件（{count}）",
        M::KeptFiles => "保留你的改动（{count}）",
        M::WrittenFiles => "写入的配置文件和其他文件（{count}）",
        M::ModpackUpdateSafety => {
            "你自己的模组、世界和改动过的配置文件不会受影响。每个被替换或移除的文件都会先保存到实例的备份文件夹中。"
        }
        M::BackUpEveryWorld => "先备份所有世界",

        M::SaveChangesTitle => "保存更改？",
        M::UnsavedDefaultsBody => "你更改了实例默认设置。要保存还是放弃？",
        M::UnsavedInstanceBody => "你更改了“{name}”的设置。要保存还是放弃？",
        M::UnsavedThisInstanceBody => "你更改了此实例的设置。要保存还是放弃？",
        M::UnsavedInDefaults => "实例默认设置中有未保存的更改",

        M::GroupLauncher => "启动器",
        M::InstanceDefaults => "实例默认设置",
        M::SharedGameSettings => "共享游戏设置",
        M::General => "通用",
        M::Accounts => "账户",
        M::Developer => "开发者",
        M::DefaultsLead => "每个实例都可以在自己的设置中更改。",
        M::SharedLead => "所有实例使用相同的游戏选项。",
        M::NoAccount => "无账户",
        M::ManageAccounts => "管理账户",
        M::GroupAutomatically => "自动分组实例",
        M::GroupAutomaticallyHint => "按加载器为未分组的实例分组。",
        M::OfflineAccount => "离线账户",
        M::MicrosoftAccount => "Microsoft 账户",
        M::SignInAgain => "重新登录",
        M::InUse => "使用中",
        M::Use => "使用",
        M::AddAnAccount => "添加账户",
        M::AddMicrosoftEllipsis => "Microsoft 账户…",
        M::AddOfflineEllipsis => "离线账户…",

        M::Memory => "内存",
        M::MemoryError => "内存：{error}",
        M::MemoryDefaultHint => "默认：{min} – {max}",
        M::MegabytesUnit => "MB",
        M::Window => "窗口",
        M::Fullscreen => "全屏",
        M::JvmArguments => "JVM 参数",
        M::GameArguments => "游戏参数",
        M::EnvironmentVariables => "环境变量",

        M::SyncedFrom => "现在，“{name}”中游戏内的更改会同步到所有实例。",
        M::SyncedFromNone => "现在没有实例会更新共享设置，请在这里编辑。",
        M::SharedCleared => "共享值已清除。各实例保留自己的 options.txt。",
        M::NoInstance => "无实例",
        M::EditHereOnly => "仅在此处编辑",
        M::Value => "值",
        M::SyncFrom => "同步来源",
        M::SyncFromHint => "在此实例游戏内的更改会同步到其他实例。",
        M::SyncFromNoneHint => "共享值只在这里更改。",
        M::StartFromInstance => "从某个实例开始",
        M::CopyFromInstance => "从实例复制…",
        M::ClearShared => "清除共享值",
        M::ClearSharedHint => "各实例保留自己的 options.txt。",
        M::ClearEveryShared => "清除所有共享值",
        M::ClearEllipsis => "清除…",
        M::NoValuesYet => "还没有值",
        M::Hide => "隐藏",
        M::Show => "显示",
        M::ServerList => "服务器列表",
        M::ServerListHint => "即将推出。",
        M::Source => "来源",
        M::WhatsShared => "共享内容",
        M::WhatsSharedLead => "关闭的类别仍由各实例单独设置。",
        M::CategoryControls => "控制",
        M::CategoryVideo => "视频",
        M::CategorySound => "声音",
        M::CategoryAccessibility => "辅助功能与聊天",
        M::CategoryLanguage => "语言",
        M::CategoryControlsHint => "鼠标、按键",
        M::CategoryVideoHint => "视场角、渲染距离、界面尺寸、图像品质",
        M::CategorySoundHint => "音量、字幕",
        M::CategoryAccessibilityHint => "复述功能、聊天、文本背景",
        M::CategoryLanguageHint => "游戏语言和字体",
        M::NarratorSection => "复述功能",
        M::KeepNarratorOff => "始终关闭复述功能",
        M::KeepNarratorOffHint => "同时跳过首次启动时的辅助功能界面。",
        M::NarratorNotShared => "“{category}”未共享，因此每个实例保留自己的复述功能设置。",
        M::NarratorKeptOff => "复述功能在每个实例中都保持关闭，包括新实例。",
        M::NarratorPerInstance => "各实例恢复使用自己的复述功能设置。",

        M::SigningIn => "正在登录…",
        M::OfflineCouldntConnect => "离线 – 无法连接",
        M::AddMinecraftAccount => "添加 Minecraft 账户",
        M::Offline => "离线",
        M::AddMicrosoftAccount => "添加 Microsoft 账户",
        M::AddOfflineAccount => "添加离线账户",
        M::RemoveNamed => "移除“{name}”",
        M::OfflineNote => "离线 – 仅限单人游戏和离线模式服务器",

        M::OfflineDialogBody => {
            "用于开发和测试。此账户没有 Microsoft 登录，其 UUID 与离线模式服务器为该名称分配的相同。"
        }
        M::Username => "用户名",
        M::UsernameRule => "3 到 16 个字母、数字或下划线。",
        M::AddAccount => "添加账户",

        M::LauncherIdentity => "启动器身份",
        M::SignInAsLauncher => "以 Minecraft Launcher 身份登录",
        M::SignInAsLauncherHint => "在 Lodestone 自己的应用获批前使用。仅供测试。",
        M::SignInAsLauncherTitle => "以 Minecraft Launcher 身份登录？",
        M::WhatThisDoes => "此选项的作用",
        M::WhatThisDoesBody => {
            "Lodestone Launcher 会使用 Mojang 官方 Minecraft Launcher 的公开客户端 ID，向 Microsoft 声称自己就是该启动器，而不是使用 Lodestone 自己注册的应用。Microsoft 和 Mojang 并未批准这种做法，Lodestone 与它们没有关联，而且这可能违反 Microsoft 对其应用的条款。"
        }
        M::Risks => "风险",
        M::RisksBody => {
            "Microsoft 随时可能阻止这种做法。一旦阻止，以这种方式登录的账户将无法使用。在 Lodestone 的应用获得批准之前，请仅用于个人测试。"
        }
        M::WhyHere => "为什么提供此选项",
        M::WhyHereBody => {
            "Lodestone 自己的应用仍在等待 Mojang 批准，因此正常的 Microsoft 登录暂时无法完成。"
        }
        M::WhenSignIn => "登录时",
        M::WhenSignInBody => {
            "“{add}”会给你一个代码。在 microsoft.com/link 输入该代码，并在那里登录你的 Microsoft 账户。Microsoft 的页面可能会显示 Minecraft 的名称，而不是 Lodestone 的。如果代码无效，你也可以改用链接登录：浏览器会打开 login.live.com，你需要将它最终停留的空白页面的地址粘贴回来。"
        }
        M::WhatLauncherGets => "启动器会获得什么",
        M::WhatLauncherGetsBody => {
            "与正常登录相同：让它能以该账户玩 Minecraft 的令牌。长期有效的令牌保存在你的{keychain}中，其余的只保存在内存里。这些账户会在账户菜单中标记为“{badge}”。"
        }
        M::HowToUndo => "如何撤销",
        M::HowToUndoBody => {
            "关闭此设置，并让这些账户退出登录。关闭时，Lodestone Launcher 会提示你让它们退出登录。"
        }
        M::TurnItOn => "我已了解，开启",
        M::TurnOffLauncherTitle => "关闭以 Minecraft Launcher 身份登录？",
        M::TurnOffLauncherBody => "新的 Microsoft 登录将重新使用 Lodestone 自己的应用。",
        M::OneLauncherAccount => "“{name}”正以 Minecraft Launcher 身份登录。",
        M::TurnOffLauncherAccounts => {
            "{accounts}保留的账户会继续可用，直到 Microsoft 阻止这种做法。退出登录会将它们从 Lodestone Launcher 中移除，并从你的{keychain}中删除其保存的登录信息。"
        }
        M::TurnOffAndSignOut => "关闭并退出登录",
        M::TurnOffOnly => "仅关闭",
        M::SignInSection => "Microsoft 登录",
        M::CommandLineSection => "命令行",
        M::CommandLineLead => {
            "lodestone-launcher-cli 和你运行的其他程序通过启动器数据文件夹中的套接字与它通信。只有你的用户账户可以连接。这些设置只能在这里更改，不能通过命令行更改。"
        }
        M::AllowCli => "允许命令行连接",
        M::AllowCliHint => "关闭后，套接字会关闭，已连接的程序会被断开。",
        M::CliAccess => "权限",
        M::CliAccessHint => "只读会回答查询，但拒绝任何会更改实例、设置、账户或文件的操作。",
        M::CliFullControl => "完全控制",
        M::CliReadOnly => "只读",
        M::DebugTraces => "调试跟踪",
        M::DebugTracesHint => {
            "记录你的点击和启动器的操作，供 lodestone-launcher-cli trace 使用。关闭后不记录任何内容。"
        }
        M::CliGameLogs => "游戏日志",
        M::CliGameLogsHint => "程序可以读取游戏的控制台输出。",
        M::SocketPath => "套接字",
        M::CopyPath => "复制路径",
        M::OpenDataFolder => "打开数据文件夹",
        M::SocketClosed => "已关闭",
        M::SocketError => "套接字无法打开：{error}",
        M::ConnectedClients => "已连接的程序",
        M::NoClients => "没有已连接的程序。",
        M::UnnamedClient => "未命名的程序",
        M::ClientSince => "连接于 {time}",
        M::ClientPid => "进程 {pid}",
        M::Disconnect => "断开连接",
        M::CliUnavailable => "Windows 上暂不支持命令行控制。",
        M::DiagnosticsSection => "诊断",
        M::PerfMonitor => "性能监视器",
        M::PerfMonitorHint => "一个小窗口，每秒更新启动器及正在运行的游戏的内存、CPU 和帧时间。",
        M::PerfOpen => "打开监视器",
        M::PerfLauncher => "启动器",
        M::PerfMemory => "内存",
        M::PerfCpu => "CPU",
        M::PerfCpuValue => "单核的 {percent} · 共 {cores} 核",
        M::PerfThreads => "线程",
        M::PerfOpenFiles => "打开的文件",
        M::PerfRendering => "渲染",
        M::PerfFps => "每秒帧数",
        M::PerfFrameTime => "帧时间",
        M::PerfFrameStats => "平均 {avg} · p95 {p95} · 最长 {max}",
        M::PerfIdle => "空闲：最近 2 秒没有重绘",
        M::PerfGame => "游戏",
        M::PerfNoGame => "没有正在运行的游戏。",
        M::PerfInternals => "启动器内部",
        M::PerfImages => "已解码的图像",
        M::PerfContentCache => "内容缓存",
        M::PerfJobs => "任务",
        M::PerfDownloads => "下载",
        M::PerfCliClients => "CLI 客户端",
        M::PerfTraceBus => "跟踪总线",
        M::PerfTraceValue => "{subscribers} 个订阅者 · 每秒 {rate} 个事件",
        M::PerfRate => "{rate}/秒",
        M::PerfSelfNote => "此窗口打开时每秒采样一次。它也会测量自身：它自己的重绘会增加少量 CPU。",
        M::PerfCopy => "复制快照",
        M::PerfCopied => "已复制",
        M::PerfLastMinute => "最近 60 秒",
        M::PerfUnknown => "–",
        M::PerfOff => "关闭",

        M::SignInWithMicrosoft => "使用 Microsoft 登录",
        M::LauncherIdentityNote => {
            "开发者选项：以 Minecraft Launcher 身份登录，而不是以 Lodestone Launcher 身份。Microsoft 的页面可能会显示 Minecraft 的名称，而不是 Lodestone 的。"
        }
        M::UseLinkInstead => "改用登录链接",
        M::UseCodeInstead => "改用代码",
        M::SignInCodeBody => {
            "使用你玩 Minecraft: Java Edition 的 Microsoft 账户登录。你会获得一个代码，需在 Microsoft 网站上输入。"
        }
        M::SignInBrowserBody => {
            "使用你玩 Minecraft: Java Edition 的 Microsoft 账户登录。浏览器会打开以完成登录。"
        }
        M::KeepsSignIn => "Lodestone Launcher 会将你的登录信息保存在{keychain}中。",
        M::GetCode => "获取代码",
        M::WaitingForBrowser => "正在等待浏览器…",
        M::FinishInBrowser => "请在打开的浏览器窗口中完成登录，然后回到这里。",
        M::OpenBrowserAgain => "重新打开浏览器",
        M::GettingCode => "正在获取代码…",
        M::GoToHost => "在任意设备上前往 {host} 并输入此代码：",
        M::CodeCopiedWaiting => "代码已复制。正在等待你输入…",
        M::WaitingForCode => "正在等待你输入代码…",
        M::CopyCodeAndOpen => "复制代码并打开 {host}",
        M::LinkInstructions => {
            "请在打开的浏览器窗口中登录。随后 Microsoft 会显示一个页面，说明该页面通常不会显示。从浏览器地址栏复制完整地址，并且只粘贴到本启动器中。"
        }
        M::LinkStartsWith => "地址以 {url} 开头",
        M::SigningInToMinecraft => "正在登录 Minecraft…",
        M::ExpectedUntilApproved => {
            "在 Mojang 批准该应用之前，这是正常现象。你的账户没有任何问题。"
        }

        M::AuthCancelled => "登录已取消",
        M::AuthCancelledBody => "登录已取消。",
        M::AuthTimedOut => "登录超时",
        M::AuthTimedOutBody => "浏览器登录未能及时完成。请重试，或改用代码。",
        M::AuthStateMismatch => "登录请求不匹配",
        M::AuthStateMismatchBody => {
            "浏览器返回了另一个登录请求，因此 Lodestone Launcher 忽略了它。请重试。"
        }
        M::AuthDeclined => "登录被拒绝",
        M::AuthDeclinedBody => "登录在浏览器中被拒绝。请重试并接受所需权限以继续。",
        M::AuthMicrosoft => "Microsoft 登录失败",
        M::AuthMicrosoftBody => "Microsoft 无法让你登录（{code}）。请稍后重试。",
        M::AuthMicrosoftSaid => "Microsoft 表示：{description}（{code}）",
        M::AuthCodeExpired => "代码已过期",
        M::AuthCodeExpiredBody => "代码在登录完成前已过期。请获取新代码后重试。",
        M::AuthSignInExpiredBody => {
            "你保存的 Microsoft 登录已过期或被撤销。请重新登录以继续使用此账户。"
        }
        M::AuthNoXbox => "没有 Xbox 个人资料",
        M::AuthNoXboxBody => {
            "此 Microsoft 账户还没有 Xbox 个人资料。请先在 xbox.com 登录一次以创建，然后重试。"
        }
        M::AuthRegion => "Xbox Live 不可用",
        M::AuthRegionBody => "此账户所在的国家或地区无法使用 Xbox Live，因此无法登录 Minecraft。",
        M::AuthAdult => "需要成人验证",
        M::AuthAdultBody => "此账户需要先在 Xbox 网站上完成成人验证才能游玩。这是韩国的要求。",
        M::AuthChild => "儿童账户",
        M::AuthChildBody => "这是一个儿童账户。需要由成人将其添加到 Microsoft 家庭组后才能登录。",
        M::AuthXbox => "Xbox Live 登录失败",
        M::AuthXboxBody => "Xbox Live 拒绝了登录（错误 {code}）。请稍后重试。",
        M::AuthNotApproved => "尚未获得 Minecraft 批准",
        M::AuthNotApprovedBody => {
            "Microsoft 登录正常，但 Minecraft 尚未批准 Lodestone Launcher。目前请使用离线模式。"
        }
        M::AuthNotOwned => "未找到 Minecraft",
        M::AuthNotOwnedBody => {
            "此 Microsoft 账户没有 Minecraft: Java Edition。如果你通过 Xbox Game Pass 游玩，请先在官方启动器中启动一次 Minecraft，然后重试。"
        }
        M::AuthNoProfile => "没有 Minecraft 个人资料",
        M::AuthNoProfileBody => {
            "此账户还没有 Minecraft 个人资料。如果你通过 Xbox Game Pass 游玩，请先在官方启动器中启动一次 Minecraft 并选择名称，然后重试。"
        }
        M::AuthRateLimited => "尝试次数过多",
        M::AuthRateLimitedBody => "登录尝试次数过多。请等待几分钟后重试。",
        M::AuthNetwork => "无法连接",
        M::AuthNetworkBody => "Lodestone Launcher 无法连接到登录服务器。请检查网络连接后重试。",
        M::AuthUnexpected => "登录失败",
        M::AuthUnexpectedBody => "“{step}”过程中出现问题（HTTP {status}）。请稍后重试。",
        M::AuthListener => "无法开始登录",
        M::AuthListenerBody => {
            "Lodestone Launcher 无法打开浏览器返回时使用的本地页面。请改用代码。"
        }
        M::AuthKeychain => "无法保存登录信息",
        M::AuthKeychainBody => "Lodestone Launcher 无法将登录信息保存到你的{keychain}。请重试。",
        M::AuthInvalidLink => "地址不正确",
        M::AuthNotAUrlBody => "这不是网址。请在登录后从浏览器地址栏复制完整地址。",
        M::AuthWrongPageBody => {
            "只接受以 {url} 开头的地址。请在登录后，从 Microsoft 显示的空白页面的地址栏中复制。"
        }
        M::AuthNoCodeBody => "该地址中没有登录代码。请先完成登录，然后复制最终停留页面的地址。",
        M::StepMicrosoftToken => "Microsoft 登录",
        M::StepDeviceCode => "Microsoft 设备代码登录",
        M::StepXboxLive => "Xbox Live 登录",
        M::StepXsts => "Xbox Live 授权",
        M::StepMinecraftLogin => "Minecraft 登录",
        M::StepEntitlements => "Minecraft 所有权检查",
        M::StepProfile => "加载 Minecraft 个人资料",

        M::AccountsTooNew => {
            "你的账户由较新版本的 Lodestone Launcher 保存。在这里所做的更改将不会被保存。"
        }
        M::AccountsCorrupt => {
            "无法读取 accounts.json，因此 Lodestone Launcher 在没有账户的情况下启动。"
        }
        M::AccountsCorruptMoved => {
            "无法读取 accounts.json，因此 Lodestone Launcher 在没有账户的情况下启动。旧文件位于 {path}。"
        }
        M::CouldntReadAccounts => "无法读取你的账户：{error}",
        M::CouldntSaveSettings => "无法保存你的设置：{error}",
        M::CouldntSaveAccounts => "无法保存你的账户：{error}",
        M::CouldntRemoveSignIn => "无法移除“{name}”保存的登录信息：{error}",
        M::AddAccountFirst => "请先添加账户：使用 Microsoft 登录，或添加离线账户。",
        M::SignInExpired => "请重新登录“{name}”：其保存的登录信息已过期。",
        M::LauncherClosing => "启动器正在关闭。",

        M::UpdatesSection => "更新",
        M::UpdateAutomatically => "自动更新",
        M::UpdateAutomaticallyHint => "在后台从 GitHub 下载新构建。",
        M::BuildNumber => "构建 {build}",
        M::BuildNumberCommit => "构建 {build} · {commit}",
        M::DevelopmentBuild => "开发构建",
        M::UpdatesUnavailable => "此构建未在 GitHub 上发布，因此不会自动更新。",
        M::UpdateNotChecked => "每隔几小时检查一次 GitHub 上是否有新构建。",
        M::UpdateChecking => "正在检查新构建…",
        M::UpToDate => "当前已是最新构建。",
        M::UpdateAvailable => "构建 {build} 现已可用。",
        M::UpdateDownloading => "正在下载构建 {build}… {percent}",
        M::UpdateReady => "构建 {build} 已安装，重启后即可使用。",
        M::UpdateReadyPlaying => "构建 {build} 已安装，关闭游戏后即可重启。",
        M::UpdateFailed => "无法更新：{error}",
        M::CheckForUpdates => "立即检查",
        M::DownloadUpdate => "下载",
        M::RestartToUpdate => "重启",
        M::UpdateReadyBadge => "更新已就绪",
        M::UpdateAvailableBadge => "有可用更新",
        M::MinimizeWindow => "最小化",
        M::MaximizeWindow => "最大化",
        M::RestoreWindow => "还原",

        M::RestOnModrinth => "完整简介请在 Modrinth 上查看。",
        M::Storage => "存储",
        M::Skin => "皮肤",
        M::SkinGallery => "皮肤",
        M::SkinImport => "导入…",
        M::SkinImportZip => "以 zip 导入皮肤",
        M::SkinImportFolder => "以文件夹导入皮肤",
        M::SkinImportButton => "导入",
        M::SkinExport => "导出…",
        M::SkinNew => "新建皮肤…",
        M::SkinDelete => "删除",
        M::SkinDeleteConfirm => "删除 {name}",
        M::SkinCopyName => "{name} 副本",
        M::SkinCopied => "已复制 {name} 以便修改。",
        M::SkinImporting => "正在导入…",
        M::SkinExporting => "正在导出…",
        M::SkinImported => "已导入 {name}。",
        M::SkinExported => "已导出到 {path}。",
        M::SkinSafeMode => "已在不使用皮肤的情况下启动（--no-skin）。",
        M::SkinCouldntLoad => "无法加载皮肤：{error}",
        M::SkinSidebar => "侧边栏",
        M::SkinImage => "图片",
        M::SkinChoose => "选择…",
        M::SkinUseImage => "使用图片",
        M::SkinRemove => "移除",
        M::SkinFit => "填充方式",
        M::SkinFitCover => "铺满",
        M::SkinFitTile => "平铺",
        M::SkinTint => "色调",
        M::SkinTintStrength => "色调强度",
        M::SkinBlur => "模糊",
        M::SkinMaterial => "材质",
        M::SkinMaterialHint => {
            "让面板后面的内容透出来：窗口背景、页面或对话框。模糊和磨砂会将其模糊显示。"
        }
        M::SkinMaterialTranslucent => "半透明",
        M::SkinMaterialBlur => "模糊",
        M::SkinMaterialFrost => "磨砂",
        M::SkinMaterialOpacity => "填充显示",
        M::SkinMaterialBlurAmount => "背景模糊",
        M::SkinMaterialFrostVeil => "磨砂层",
        M::SkinMaterialGrain => "颗粒",
        M::SkinBanner => "实例横幅",
        M::SkinBannerInSidebar => "显示在侧边栏条目后面",
        M::SkinHero => "“开始游戏”横幅",
        M::SkinHeroHint => "用于没有自己图片的实例。",
        M::SkinPlayButton => "“开始游戏”按钮",
        M::SkinColor => "颜色",
        M::SkinFadeTo => "渐变到",
        M::SkinShape => "形状",
        M::SkinShapeRounded => "圆角",
        M::SkinShapePill => "胶囊",
        M::SkinShapePixel => "像素",
        M::SkinLabel => "文字",
        M::SkinGlow => "光晕",
        M::SkinColors => "颜色",
        M::SkinAccent => "强调色",
        M::SkinProgress => "进度条",
        M::SkinDefault => "默认",
        M::SkinNone => "无",
        M::SkinAdjusted => "强调色已调整以保持可读。",
        M::SkinReset => "恢复默认",
        M::SkinPlayColor => "“开始游戏”按钮颜色",
        M::SkinUseSkins => "跟随皮肤",
        M::SkinSurfaceWindow => "窗口",
        M::SkinSurfaceTopBar => "顶栏",
        M::SkinSurfaceSidebar => "侧边栏",
        M::SkinSurfaceMain => "页面",
        M::SkinSurfaceHero => "游玩图片",
        M::SkinSurfacePlayBar => "游玩栏",
        M::SkinSurfaceCard => "卡片",
        M::SkinSurfaceSettingCard => "设置分组",
        M::SkinSurfaceRow => "列表行",
        M::SkinSurfaceDialog => "对话框",
        M::SkinSurfaceSettingsNav => "设置页面列表",
        M::SkinSurfacePopover => "菜单",
        M::SkinSurfaceInput => "文本框",
        M::SkinSurfaceButtonPrimary => "主要按钮",
        M::SkinSurfaceButtonSecondary => "次要按钮",
        M::SkinSurfaceButtonGhost => "低调按钮",
        M::SkinSurfaceButtonDanger => "危险按钮",
        M::SkinSurfaceToggle => "开关",
        M::SkinSurfaceTab => "标签页",
        M::SkinSurfacePill => "胶囊选择器",
        M::SkinSurfacePlayButton => "游玩按钮",
        M::SkinSurfaceProgress => "进度条",
        M::SkinSurfaceToast => "通知",
        M::SkinRoleText => "文字",
        M::SkinRoleMuted => "次要文字",
        M::SkinRoleFaint => "提示和图标",
        M::SkinRoleLabel => "标签",
        M::SkinCheckShort => "{surface}，{role}：{ratio}:1，需要 {target}:1",
        M::SkinCheckNoFix => "在此填充上没有文字颜色能达到；请更换填充。",
        M::SkinApplyFix => "应用建议的修正",
        M::SkinFixAll => "全部修正",
        M::SkinChecksPass => "所有文字都清晰可读（{count} 项检查）。",
        M::SkinChecksNote => "可读性由皮肤决定：以下仅为建议。",
        M::SkinBanners => "来自皮肤",
        M::SkinBannerMissing => "此横幅所属的皮肤已不存在，正在显示默认横幅。",
        M::SkinEditorTitle => "皮肤编辑器",
        M::SkinPopOut => "打开编辑器窗口",
        M::SkinEditorLead => {
            "在编辑器中修改每个界面区域、字体和横幅。修改会立即显示在窗口中，可以用撤销恢复。"
        }
        M::SkinSummary => "已设置 {surfaces} 个区域 · {fonts} 种字体 · {banners} 个横幅",
        M::SkinSurfaces => "界面区域",
        M::SkinTokens => "文字颜色",
        M::SkinFonts => "字体",
        M::SkinBannerSet => "横幅",
        M::SkinPlayButtonSection => "游玩按钮",
        M::SkinReadability => "可读性",
        M::SkinFill => "填充",
        M::SkinFillColour => "颜色",
        M::SkinFillGradient => "渐变",
        M::SkinOpacity => "不透明度",
        M::SkinGradientLinear => "线性",
        M::SkinGradientRadial => "径向",
        M::SkinAngle => "角度",
        M::SkinStops => "颜色",
        M::SkinStopAt => "位置",
        M::SkinAddStop => "添加颜色",
        M::SkinRemoveStop => "移除颜色",
        M::SkinBuiltinImages => "内置图片",
        M::SkinBorder => "边框",
        M::SkinBorderWidth => "边框宽度",
        M::SkinRadius => "圆角",
        M::SkinTextColour => "文字",
        M::SkinMutedColour => "次要文字",
        M::SkinTextElsewhere => "此区域的文字使用文字颜色设置。",
        M::SkinTokenText => "文字",
        M::SkinTokenSecondary => "标签",
        M::SkinTokenMuted => "描述",
        M::SkinTokenFaint => "提示",
        M::SkinTokenBorder => "线条",
        M::SkinFontHeading => "标题",
        M::SkinFontBody => "正文",
        M::SkinFontSidebar => "侧边栏",
        M::SkinFontButton => "按钮",
        M::SkinFontPlay => "游玩按钮和标题",
        M::SkinFontTab => "标签页",
        M::SkinFontMono => "代码和日志",
        M::SkinFontAdd => "添加字体文件…",
        M::SkinBannerAdd => "添加横幅…",
        M::SkinBannersNone => "还没有横幅。新实例使用默认的游玩图片。",
        M::SkinBannersLead => "新实例会随机获得一个；之后可在游玩背景中更换。",
        M::SkinSaveAsNew => "另存为新皮肤",
        M::SkinUseLaunchers => "使用启动器默认",
        M::SkinPreviewIn => "在窗口中显示",
        M::SkinPreviewSettings => "设置",
        M::SkinPreviewMenu => "菜单",
        M::SkinPreviewNotice => "通知",
        M::SkinPreviewNoticeText => "通知看起来是这样的。",
        M::SkinSampleText => "示例文字",
        M::SkinSampleMuted => "次要文字",
        M::SkinScrim => "标题下的阴影",
        M::StorageInstances => "实例",
        M::StorageGameFiles => "游戏文件",
        M::StorageRuntimes => "Java 运行时",
        M::StorageCache => "缓存",
        M::StorageSnapshots => "快照",
        M::StorageSnapshotsHint => "{count} 个快照 · 可释放 {size}",
        M::StorageCleanSnapshotsTitle => "移除自动快照？",
        M::StorageCleanSnapshotsBody => {
            "这会释放 {size}。将移除回退前自动拍摄的快照、已删除实例的快照以及为分享而生成的文件。"
        }
        M::StorageLeftovers => "未完成的下载和导入",
        M::StorageCleanLeftoversTitle => "移除未完成的下载？",
        M::StorageCleanLeftoversBody => {
            "这将释放 {size}。这些是未完成的整合包下载和导入留下的文件。"
        }
        M::StorageOther => "设置和其他文件",
        M::StorageCacheAndLogs => "缓存和日志",
        M::StorageLogs => "日志和崩溃报告",
        M::StorageOldLogs => "{size} 超过 30 天",
        M::StorageVersions => "版本",
        M::StorageLibraries => "库",
        M::StorageAssets => "资源",
        M::StorageUnused => "未被任何实例使用",
        M::StorageUnusedTag => "未使用",
        M::StorageJava => "Java {major}",
        M::StorageWorlds => "世界 {size}",
        M::StorageContent => "模组和资源包 {size}",
        M::StorageOtherFiles => "其他 {size}",
        M::StorageNoInstances => "没有实例",
        M::StorageNoRuntimes => "没有 Java 运行时",
        M::StorageCounting => "正在统计…",
        M::StorageCouldntCount => "无法统计文件：{error}",
        M::StorageStopGame => "请先停止游戏，再移除游戏文件、Java 运行时或缓存。",
        M::StorageOpenFolder => "打开文件夹",
        M::StorageOpen => "打开",
        M::StorageDelete => "删除…",
        M::StorageRemoveEllipsis => "移除…",
        M::StorageClearEllipsis => "清除…",
        M::StorageRemove => "移除",
        M::StorageClear => "清除",
        M::StorageDeleteOld => "删除旧的…",
        M::StorageDeleteAll => "全部删除…",
        M::StorageDeleteConfirm => "删除",
        M::StorageFreed => "已释放 {size}",
        M::StorageCouldntRemove => "无法全部移除：{error}",
        M::StorageCleanGameFilesTitle => "移除未使用的游戏文件？",
        M::StorageCleanGameFilesBody => {
            "这将释放 {size}。没有实例使用的版本、库和资源会被删除，实例需要时会重新下载。"
        }
        M::StorageCleanRuntimesTitle => "移除未使用的 Java 运行时？",
        M::StorageCleanRuntimesBody => "这将释放 {size}。Minecraft 版本需要时会重新下载。",
        M::StorageClearCacheTitle => "清除缓存？",
        M::StorageClearCacheBody => "这将释放 {size}。启动器会重新下载所需内容。",
        M::StorageDeleteOldLogsTitle => "删除旧日志？",
        M::StorageDeleteOldLogsBody => "这将释放 {size}。超过 30 天的日志和崩溃报告会被永久删除。",
        M::StorageDeleteAllLogsTitle => "删除所有日志？",
        M::StorageDeleteAllLogsBody => {
            "这将释放 {size}。所有日志和崩溃报告都会被永久删除，正在运行的游戏的除外。"
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AndMore => ["还有 {count} 个"; 2],
        P::Values => ["{count} 个值"; 2],
        P::CopiedValues => ["已从“{name}”复制 {count} 个值。"; 2],
        P::LauncherAccounts => ["有 {count} 个账户正以 Minecraft Launcher 身份登录：{names}。"; 2],
        P::Subscriptions => ["{count} 个订阅"; 2],
        P::PerfEntries => ["{count} 个条目"; 2],
    }
}
