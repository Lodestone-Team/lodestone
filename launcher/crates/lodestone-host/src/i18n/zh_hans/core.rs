use crate::i18n::{CoreMsg as M, CorePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::SectionCore => "核心",
        M::SectionPublic => "公开服务器",
        M::SectionPublicLead => "无需登录即可查看。",
        M::SectionInfo => "信息",
        M::SectionDanger => "危险区域",
        M::SectionDangerLead => "请谨慎更改。",
        M::CoreName => "核心名称",
        M::CoreNameEmpty => "名称不能为空",
        M::CoreNameTooLong => "名称不能超过 {max} 个字符",
        M::Domain => "公网域名或 IP",
        M::DomainTooLong => "域名不能超过 {max} 个字符",
        M::DomainHint => "如果你已设置端口转发，这里填写你的公网 IP。否则请保持不变。",
        M::ListPublicly => "公开列出服务器",
        M::ListPubliclyHint => "任何能访问此核心的人都能看到你在下方设为公开的服务器。",
        M::LocalClaim => "允许本机上的应用获取所有者权限",
        M::LocalClaimHint => {
            "Lodestone Launcher 可以通过证明自己能写入此核心的数据文件夹来成为所有者。能写入该文件夹的程序本来就能修改所有用户和设置，所以这只在共用电脑上有影响。"
        }
        M::SafeMode => "安全模式",
        M::SafeModeHint => "仅所有者可获得此计算机的管理权限。",
        M::SafeModeOffTitle => "关闭安全模式？",
        M::SafeModeOffBody => {
            "确定要关闭安全模式吗？这样你就可以让自己以外的用户运行可能有危险的命令。请确保你信任所有获得这些权限的用户。"
        }
        M::TurnOff => "关闭",
        M::CodeExecution => "代码执行",
        M::CodeExecutionHint => "宏、Atom 服务器、扩展、服务器脚本和自定义 Java 命令。",
        M::CodeExecutionHintJavaOnly => "自定义 Java 命令。",
        M::CodeExecutionLocked => {
            "已被此核心的管理员锁定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。"
        }
        M::CodeExecutionOffTitle => "关闭代码执行？",
        M::CodeExecutionOffBody => {
            "正在运行的宏和 Atom 服务器将被停止。在你重新开启代码执行之前，宏、Atom 服务器、扩展、服务器脚本和自定义 Java 命令都不会在此核心上运行。任何内容都不会自动重新启动。"
        }
        M::CodeExecutionOffBodyJavaOnly => {
            "在你重新开启代码执行之前，自定义 Java 命令不会在此核心上运行。任何内容都不会自动重新启动。"
        }
        M::NoScripting => "此核心构建时未包含脚本支持（Deno），因此无法使用宏。",
        M::LanHttp => "允许通过明文 HTTP 进行局域网访问",
        M::LanHttpHint => "未加密。你网络中的其他人可以读取密码。",
        M::LanHttpConfirmTitle => "允许其他设备使用明文 HTTP？",
        M::LanHttpConfirmBody => {
            "你网络中的设备将能够通过明文 HTTP 连接。以这种方式发送的密码和会话令牌会以未加密的方式经过网络，网络中的其他任何人都可以读取它们并接管此核心上的账户。仅在某台设备无法使用 https:// 时才允许此项。"
        }
        M::LanHttpAllow => "允许",
        M::LanHttpWarningOwner => {
            "其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请在下方关闭“{setting}”，并改用 https:// 连接。"
        }
        M::LanHttpWarningOther => {
            "其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请使用 https:// 连接，并请所有者关闭明文 HTTP 访问。"
        }
        M::OpenPort => "开放端口",
        M::OpenPortHint => "使用 {upnp} 在路由器上设置端口转发。",
        M::OpenPortNoPermission => "没有权限",
        M::OpenPortButton => "开放端口",
        M::OpenPortPort => "要开放的端口",
        M::PortNotNumber => "端口必须是数字",
        M::PortRequired => "请输入端口",
        M::PortRange => "端口必须在 1 到 65535 之间",
        M::PortOpened => "已开放端口 {port}",
        M::InfoCoreVersion => "核心版本",
        M::InfoHostVersion => "Lodestone Host 版本",
        M::InfoOs => "操作系统",
        M::InfoArch => "架构",
        M::InfoCpu => "CPU",
        M::InfoCpuCores => "CPU 核心数",
        M::InfoTotalRam => "总内存",
        M::InfoNoRam => "没有内存？你是怎么运行 Lodestone 的？",
        M::InfoHttps => "HTTPS 证书",
        M::InfoFingerprint => "证书指纹（SHA-256）",
        M::TlsSelfSigned => "内置自签名证书",
        M::TlsUserProvided => "你自己的证书（tls/cert.pem）",
        M::TlsNone => "不可用，仅明文 HTTP",
        M::ReadOnly => "只有所有者可以更改这些设置。",
        M::ListingTitle => "共享",
        M::ListingLead => "谁可以在不登录的情况下找到此服务器。",
        M::ListedPublicly => "公开列出",
        M::ListingPublic => "公开",
        M::ListingOwnerOnly => "只有所有者可以更改此项",
        M::FlagStatus => "状态和玩家",
        M::FlagAddress => "加入地址",
        M::FlagClientPack => "客户端模组下载",
        M::ShownText => "任何能访问此核心的人无需登录即可看到此服务器的{list}。",
        M::ShownNameVersion => "名称和版本",
        M::ShownStatus => "状态和玩家数",
        M::ShownAddress => "地址",
        M::ShownClientPack => "客户端模组",
        M::TurnOnFirst => "请先开启“公开列出服务器”。",
        M::HiddenWhileOff => "{shown}“公开列出服务器”关闭时隐藏。",
        M::NotShown => "未登录时不显示。",
        M::NoJavaServers => "还没有 Minecraft Java Edition 服务器。",
        M::UsersDescription => "此核心上的账户及其权限。",
        M::AllUsers => "所有用户",
        M::AllMembers => "所有成员",
        M::AllMembersCount => "所有成员（{count}）",
        M::CreateUser => "创建新用户",
        M::CreateUserTitle => "创建新用户账户",
        M::CreateUserLead => "新用户初始没有任何权限。",
        M::LoadingUsers => "正在加载用户",
        M::Permissions => "权限",
        M::PermissionsLead => "{username} 在此核心上可以执行的操作。",
        M::UnsafePermissions => "不安全权限",
        M::UnsafePermissionsLead => "关闭安全模式后才能授予非所有者。",
        M::OwnerOnly => "宏、扩展和 Atom 服务器",
        M::OwnerOnlyHint => "仅限所有者。Docker 服务器使用普通权限。",
        M::UsersOwnerOnly => "只有所有者可以管理用户。",
        M::You => "你",
        M::Admin => "管理员",
        M::Owner => "所有者",
        M::ChangePasswordMenu => "修改密码",
        M::CantDeleteSelf => "你不能删除自己。",
        M::DeleteUserTitle => "删除用户",
        M::DeleteUserConfirm => "确定要删除 {name} 吗？此操作无法撤销。",
        M::DeleteUser => "删除用户",
        M::NoServersYet => "还没有服务器。",
        M::PermCreateInstance => "创建服务器",
        M::PermCreateInstanceUser => "该用户可以创建新服务器。",
        M::PermCreateInstanceKey => "该密钥可以创建新服务器。",
        M::PermDeleteInstance => "删除服务器",
        M::PermDeleteInstanceUser => "该用户可以删除任何服务器。",
        M::PermDeleteInstanceKey => "该密钥可以删除任何服务器。",
        M::PermReadGlobalFile => "读取全局文件",
        M::PermReadGlobalFileUser => "该用户可以读取运行 Lodestone Core 的计算机上的任何文件。",
        M::PermReadGlobalFileKey => "该密钥可以读取运行 Lodestone Core 的计算机上的任何文件。",
        M::PermViewInstance => "查看服务器",
        M::PermViewInstanceUser => "该用户可以看到这些服务器。",
        M::PermViewInstanceKey => "该密钥可以看到这些服务器。",
        M::PermStartInstance => "启动服务器",
        M::PermStartInstanceUser => "该用户可以启动这些服务器。",
        M::PermStartInstanceKey => "该密钥可以启动这些服务器。",
        M::PermStopInstance => "停止服务器",
        M::PermStopInstanceUser => "该用户可以停止这些服务器。",
        M::PermStopInstanceKey => "该密钥可以停止这些服务器。",
        M::PermConsole => "访问服务器控制台",
        M::PermConsoleUser => {
            "该用户可以读取这些服务器的控制台并向其发送命令。这实际上相当于授予服务器的管理员（OP）权限。"
        }
        M::PermConsoleKey => {
            "该密钥可以读取这些服务器的控制台并向其发送命令。这实际上相当于授予服务器的管理员（OP）权限。"
        }
        M::PermInstanceSettings => "访问服务器设置",
        M::PermInstanceSettingsUser => "该用户可以读取和更改这些服务器的设置。",
        M::PermInstanceSettingsKey => "该密钥可以读取和更改这些服务器的设置。",
        M::PermReadResource => "读取服务器资源",
        M::PermReadResourceUser => "该用户可以读取这些服务器的资源（世界、模组等）。",
        M::PermReadResourceKey => "该密钥可以读取这些服务器的资源（世界、模组等）。",
        M::PermReadFiles => "读取服务器文件",
        M::PermReadFilesUser => "该用户可以读取这些服务器的文件。",
        M::PermReadFilesKey => "该密钥可以读取这些服务器的文件。",
        M::PermWriteResource => "修改服务器资源",
        M::PermWriteResourceUser => "该用户可以修改这些服务器的资源（世界、模组等）。",
        M::PermWriteResourceKey => "该密钥可以修改这些服务器的资源（世界、模组等）。",
        M::PermWriteFiles => "修改服务器文件",
        M::PermWriteFilesUser => "该用户可以修改这些服务器的文件。",
        M::PermWriteFilesKey => "该密钥可以修改这些服务器的文件。",
        M::PermWriteGlobalFile => "修改全局文件",
        M::PermWriteGlobalFileUser => "该用户可以修改运行 Lodestone Core 的计算机上的任何文件。",
        M::PermWriteGlobalFileKey => "该密钥可以修改运行 Lodestone Core 的计算机上的任何文件。",
        M::PermManagePermission => "管理权限",
        M::PermManagePermissionUser => "该用户可以管理其他用户的权限。",
        M::PermManagePermissionKey => "该密钥可以管理其他用户的权限。",
        M::Username => "用户名",
        M::Password => "密码",
        M::OldPassword => "旧密码",
        M::ConfirmPassword => "确认密码",
        M::UsernameRequired => "请输入用户名",
        M::PasswordRequired => "请输入密码",
        M::ConfirmRequired => "请确认密码",
        M::PasswordsMustMatch => "两次输入的密码不一致",
        M::ChangePasswordButton => "更改密码",
        M::CreateUserButton => "创建用户",
        M::Account => "账户",
        M::ProfileUsername => "用户名",
        M::ProfilePassword => "密码",
        M::ChangePassword => "更改密码",
        M::Appearance => "外观",
        M::AppearanceLead => "保存在这台电脑上，而不是核心上。",
        M::ColoredConsole => "彩色控制台",
        M::Language => "语言",
        M::LanguageAuto => "系统默认（{language}）",
        M::Theme => "主题",
        M::ThemeSystem => "跟随系统",
        M::ThemeLight => "浅色",
        M::ThemeDark => "深色",
        M::ThemeSystemHint => "跟随电脑的浅色或深色设置",
        M::ThemeLightHint => "始终使用浅色",
        M::ThemeDarkHint => "始终使用深色",
        M::Zoom => "缩放",
        M::ZoomHint => "也可以用 {modifier} + 和 {modifier} −。",
        M::ApiKeys => "API 密钥",
        M::ApiKeysOwnLead => "让 CLI、脚本和智能体无需密码即可以你的身份操作。",
        M::ApiKeysOwnEmpty => "你还没有 API 密钥。",
        M::ApiKeysUserLead => "{username} 创建的密钥。吊销后，使用它的程序会立即失去访问权限。",
        M::ApiKeysUserEmpty => "{username} 没有 API 密钥。",
        M::CreateApiKey => "创建 API 密钥",
        M::ApiKeyCreated => "已创建 API 密钥“{name}”",
        M::LoadingApiKeys => "正在加载 API 密钥",
        M::Revoke => "吊销",
        M::RevokeTitle => "吊销 API 密钥",
        M::RevokeBody => {
            "吊销“{name}”（{display}）？使用此密钥的所有程序将立即失去访问权限。此操作无法撤销。"
        }
        M::ApiKeyRevoked => "已吊销 API 密钥“{name}”",
        M::CreatedOn => "创建于 {date}",
        M::ExpiresOn => "{date} 过期",
        M::ExpiredOn => "已于 {date} 过期",
        M::ExpiresNever => "过期时间：永不",
        M::LastUsed => "上次使用：{time}",
        M::LastUsedNever => "上次使用：从未",
        M::ScopeFull => "完全访问",
        M::ScopeLauncher => "Launcher：查看、启动和停止服务器",
        M::ScopeLauncherConsole => "Launcher：查看、启动和停止服务器，读取和使用控制台",
        M::ScopeLauncherConsoleRead => "Launcher：查看、启动和停止服务器，读取控制台",
        M::ScopeLauncherConsoleWrite => "Launcher：查看、启动和停止服务器，发送控制台命令",
        M::ScopeCustom => "自定义：{servers}，{permissions}",
        M::PasswordSessionOnly => "只有使用密码登录时才能管理 API 密钥，使用 API 密钥登录时不行。",
        M::CreateApiKeyLead => "为每个密钥设置较短的有效期，并只授予所需的访问权限。",
        M::ApiKeyName => "名称",
        M::ApiKeyNameRequired => "请输入名称",
        M::ApiKeyNameTooLong => "名称最多 {max} 个字符",
        M::ExpiresIn => "有效期",
        M::Expiry30d => "30 天",
        M::Expiry90d => "90 天",
        M::Expiry1y => "1 年",
        M::ExpiryNever => "永不过期",
        M::NeverWarning => "永不过期的密钥在你吊销之前一直有效。除非确实需要，否则请设置有效期。",
        M::Access => "访问权限",
        M::AccessFull => "完全访问（你能做的一切）",
        M::AccessCustom => "自定义",
        M::ChooseOne => "请至少选择一项权限。",
        M::CreateKey => "创建密钥",
        M::OnlyYours => "密钥只能拥有你自己拥有的权限。你没有的权限和服务器显示为灰色。",
        M::NotYours => "你没有此权限。",
        M::NoServers => "没有你可以看到的服务器。",
        M::KeyUnsafe => "不安全设置",
        M::KeyUnsafeSafeMode => {
            "安全模式已开启，因此在核心设置中关闭安全模式之前，核心会拒绝为自定义访问的密钥授予这些权限。"
        }
        M::KeyOwnerOnly => "宏、扩展和 Atom 服务器",
        M::KeyOwnerOnlyHint => "只有完全访问的密钥才能使用宏、扩展和 Atom 服务器。",
        M::NewKeyTitle => "已创建 API 密钥“{name}”",
        M::NewKeyCopyNow => "请立即复制此密钥。之后将无法再次查看。",
        M::NewKeyUseWithCli => "在 Lodestone CLI 中使用：",
        M::NewKeyStore => {
            "请像保管密码一样保管此密钥。在它过期或被你吊销之前，任何拥有它的人都可以凭它的权限使用 Lodestone。"
        }
        M::KeyCopied => "已将 API 密钥复制到剪贴板",
        M::CliCopied => "已将 CLI 示例复制到剪贴板",
        M::MinecraftAccounts => "Minecraft 账户",
        M::MinecraftOwnLead => "从 Lodestone Launcher 关联。",
        M::MinecraftOwnEmpty => {
            "尚未关联 Minecraft 账户。在 Lodestone Launcher 中登录此核心即可关联。"
        }
        M::MinecraftUserLead => "{username} 从 Lodestone Launcher 关联的 Minecraft 账户。",
        M::MinecraftUserEmpty => "{username} 没有关联的 Minecraft 账户。",
        M::MinecraftTooOld => "此核心版本太旧，无法关联 Minecraft 账户。",
        M::MinecraftRemoveTitle => "移除 Minecraft 账户",
        M::MinecraftRemoveBody => "移除与 {name} 的关联？之后可以从 Lodestone Launcher 重新关联。",
        M::MinecraftRemoved => "已移除 {name}",
        M::Notifications => "通知",
        M::InProgress => "进行中",
        M::AllNotifications => "所有通知",
        M::LoadingNotifications => "正在加载通知",
        M::NoNotifications => "暂无通知！",
        M::EventStarting => "服务器 {name} 正在启动！",
        M::EventRunning => "服务器 {name} 已启动！",
        M::EventStopping => "服务器 {name} 正在停止！",
        M::EventStopped => "服务器 {name} 已停止！",
        M::EventCrashed => "服务器 {name} 出现错误！",
        M::WarningToast => "警告：某个服务器发出了警告。这不应该发生，请向开发者报告此问题。",
        M::EventWarning => "服务器 {name} 发出了警告",
        M::EventError => "服务器 {name} 出现错误",
        M::PlayersJoined => "{players} 加入了 {name}",
        M::PlayersLeft => "{players} 离开了 {name}",
        M::PlayersJoinedAndLeft => "{joined} 加入了 {name}，{left} 离开了 {name}",
        M::PlayerMessage => "{player} 在 {name} 上说：{message}",
        M::EventApiKeyCreated => "已创建 API 密钥“{name}”",
        M::EventApiKeyRevoked => "已吊销 API 密钥“{name}”",
        M::MacroStarted => "宏 {pid} 已在 {server} 上启动",
        M::MacroDetached => "宏 {pid} 已在 {server} 上分离",
        M::MacroStopped => "宏 {pid} 已在 {server} 上停止，状态为 {status}",
        M::LoadFailed => "无法加载通知：{error}",
        M::CoreFallback => "核心",
        M::CoreFailedNotice => "Lodestone Core 无法启动：{error}",
        M::UpdatesTitle => "更新",
        M::UpdatesLead => "这台电脑上的 Lodestone Host。Lodestone Core 随之更新。",
        M::ThisVersion => "此版本",
        M::UpdateAutomatically => "自动更新",
        M::UpdateAutomaticallyHint => "在后台下载新构建。",
        M::BetaBuilds => "测试版构建",
        M::BetaBuildsHint => "测试版构建可能有错误。",
        M::DeveloperBuilds => "开发者构建",
        M::DeveloperBuildsHint => "供 Lodestone 开发者使用：更新到开发版构建，可能有错误。",
        M::BuildNumber => "构建 {build}",
        M::BuildNumberCommit => "构建 {build} · {commit}",
        M::DevelopmentBuild => "开发构建",
        M::UpdatesUnavailable => "此构建不会自动更新。从官网下载的 Lodestone Host 会自动更新。",
        M::UpdateNotChecked => "每隔几小时检查一次是否有新构建。",
        M::UpdateChecking => "正在检查新构建…",
        M::UpToDate => "当前已是最新构建。",
        M::UpdateAvailable => "构建 {build} 现已可用。",
        M::UpdateDownloading => "正在下载构建 {build}… {percent}",
        M::UpdateReady => {
            "构建 {build} 已下载，重启后即可使用：你的服务器会停止，设为自动启动的服务器随后会再次启动。退出时也会安装。"
        }
        M::RestartUpdateTitle => "重启以更新？",
        M::RestartUpdateBody => {
            "重启 Lodestone Host 会先停止这些服务器。设为自动启动的服务器会在重启后再次启动。"
        }
        M::RestartUpdateConfirm => "停止服务器并重启",
        M::DeveloperSection => "开发者",
        M::ConnectService => "Connect 服务（中继）",
        M::ConnectServiceHint => "仅供测试。邀请链接和好友列表使用此服务。更改后会重启核心。",
        M::ConnectServiceInUse => "开发中继：{service}",
        M::ConnectServiceEnv => {
            "已设置 {var}，因此核心使用 {service}。取消设置后，此设置才会生效。"
        }
        M::ConnectServiceScheme => "地址需以 https:// 开头。",
        M::ConnectServiceInsecure => {
            "请使用 https://。只有本机上的服务（127.0.0.1 或 localhost）可以使用 http://。"
        }
        M::ConnectServiceExtra => "只输入地址，不要包含路径、查询参数或用户名。",
        M::ConnectServiceAddress => "这不是有效的地址。",
        M::SaveAndRestartCore => "保存并重启核心",
        M::RestartCoreTitle => "重启核心？",
        M::RestartCoreBody => {
            "更改 Connect 服务会重启 Lodestone Core，它会先停止这些服务器。设为自动启动的服务器会在重启后再次启动。"
        }
        M::UpdateFailed => "无法更新：{error}",
        M::CheckForUpdates => "立即检查",
        M::DownloadUpdate => "下载",
        M::RestartToUpdate => "重启",
        M::BetaTitle => "感谢使用 Lodestone 测试版！",
        M::BetaBody => {
            "感谢使用 Lodestone 测试版！如果你发现任何问题，请在我们的 Discord 服务器或 Github 上告诉我们。"
        }
        M::Cores => "核心",
        M::CoresLead => {
            "Lodestone Host 运行这台电脑上的核心。你也可以管理另一个核心，例如服务器上的核心。"
        }
        M::ThisComputerCore => "这台电脑",
        M::ThisComputerHint => "Lodestone Host 运行的核心。",
        M::CurrentCore => "当前",
        M::UseCore => "切换",
        M::ChangeCore => "更换核心",
        M::ConnectToCore => "连接到核心",
        M::CoreAddress => "地址",
        M::CoreAddressHint => "例如 192.168.1.5 或 core.example.com:16662。",
        M::Connect => "连接",
        M::TrustTitle => "信任此核心的证书吗？",
        M::TrustBody => {
            "{host} 使用自己的 HTTPS 证书，这台电脑不认识它。Lodestone 核心会自己生成证书，所以这是正常的：请核对它的指纹，然后信任它。"
        }
        M::FingerprintCaption => "证书指纹（SHA-256）",
        M::FingerprintHelp => {
            "请将它与该核心“设置 › 常规 › 信息”中的指纹，或它启动时日志中的指纹进行比对。"
        }
        M::TrustCertificate => "信任证书",
        M::SignInTo => "登录到 {name}",
        M::VersionIncompatible => {
            "此 Core 运行 Lodestone Core {theirs}，本机运行 {ours}。在两者都更新之前，某些页面可能无法使用。"
        }
        M::VersionPatch => "此 Core 运行 Lodestone Core {theirs}（本机：{ours}）。",
        M::SignIn => "登录",
        M::RemoveCore => "移除",
        M::RemoteSignedOut => "你已从 {name} 退出登录。Lodestone Host 已回到这台电脑的核心。",
        M::RemoteAccess => "允许其他设备访问",
        M::RemoteAccessHintOff => {
            "关闭：只有这台电脑上的应用可以使用这个核心（Lodestone Host、Lodestone Launcher 和 Lodestone CLI）。打开后可以在其他设备上使用网页控制台、添加用户或创建 API 密钥。无论开关如何，都可以分享服务器。"
        }
        M::RemoteAccessHintOn => "其他设备可以通过 HTTPS 用用户名和密码登录这个核心。",
        M::RemoteAccessOffTitle => "关闭其他设备的访问？",
        M::RemoteAccessOffBody => {
            "只有这台电脑上的应用可以使用这个核心。其他设备、其他用户和在别处使用的 API 密钥将无法使用，直到你重新打开。分享服务器仍然可用。"
        }
        M::LocalClaimNeedsPassword => "请先设置所有者账户：没有密码时，这是唯一的登录方式。",
        M::OwnerAccountTitle => "设置所有者账户以分享这个核心",
        M::OwnerAccountLead => {
            "其他设备使用用户名和密码登录。请选择你将在网页控制台和其他设备上使用的用户名和密码。这台电脑上的应用无需它们也能继续登录。"
        }
        M::OwnerAccountButton => "设置并允许访问",
        M::OwnerAccountDone => "所有者账户已设置。其他设备现在可以登录这个核心。",
        M::PasswordTooShort => "密码至少需要 12 个字符。",
        M::SetPassword => "设置密码",
        M::NoPasswordYet => "还没有密码。只有这台电脑上的应用可以使用这个核心。",
        M::RemoteAccessKeywords => "远程 局域网 网络 共享",
        M::DomainKeywords => "地址 主机名 网址",
        M::OpenPortKeywords => "端口转发 路由器 UPnP 防火墙",
        M::ThemeKeywords => "深色 浅色 模式 外观 颜色",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScopeServers => ["{count} 个服务器", "{count} 个服务器"],
        P::ScopePermissions => ["{count} 项权限", "{count} 项权限"],
        P::ServersRunning => ["{count} 个服务器正在运行。", "{count} 个服务器正在运行。"],
    }
}
