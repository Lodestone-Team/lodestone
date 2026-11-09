use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::PageHint => "你的 Lodestone 核心上的服务器。创建与其中一个匹配的实例，然后在上面游玩。",
        M::OnThisComputer => "这台电脑上",
        M::OnThisComputerHint => {
            "在这里运行的 Lodestone Host 和 Lodestone Core。以所有者身份连接时，启动器会证明它能访问核心的文件，因此不需要密码。"
        }
        M::Rescan => "重新扫描",
        M::ScanningLocal => "正在查找这台电脑上的核心…",
        M::NoLocalCores => "这台电脑上没有正在运行的 Lodestone Host 或 Lodestone Core。",
        M::ConnectAsOwner => "以所有者身份连接",
        M::Connected => "已连接",
        M::ChooseDataFolder => "选择文件夹…",
        M::ChooseDataFolderPrompt => "选择",
        M::ProductHostHint => "桌面应用，控制面板在窗口中",
        M::ProductCoreHint => "独立的核心（服务器程序或 Docker）",
        M::ConnectedAsOwner => "已以所有者身份连接到 {name}",
        M::ConnectedMadeOwner => {
            "它还没有所有者，因此 Lodestone Launcher 创建了所有者账户 {username}。密码显示在核心的卡片上，请妥善保存。"
        }
        M::ConnectedOwnerKey => "此启动器现在拥有它的所有者密钥。你可以在核心的控制面板中撤销它。",
        M::OwnerSignIn => "所有者登录",
        M::OwnerKey => "所有者密钥",
        M::ShowPassword => "显示",
        M::HidePassword => "隐藏",
        M::CopyUsername => "复制用户名",
        M::CopyPassword => "复制密码",
        M::CopyApiKey => "复制 API 密钥",
        M::StartedBy => "由 {who} 启动",
        M::StartedDirectly => "直接启动",
        M::DirectNote => {
            "此核心是直接启动的。Lodestone Core 应通过 lodestone_cli 运行，它会安装核心并保持更新。"
        }
        M::GetLodestoneCli => "获取 lodestone_cli",
        M::CredentialsReadFailed => "无法读取这台电脑上核心的所有者密钥",
        M::CredentialsSaveFailed => "无法保存所有者密钥",
        M::ClaimNoCore => "这台电脑上没有正在运行的 Lodestone Host 或 Lodestone Core。",
        M::ClaimRefused => {
            "核心拒绝授予所有者权限。请在它的设置中打开“允许这台电脑上的应用获取所有者权限”，或更新它。"
        }
        M::ClaimFolderNotFound => {
            "找不到此核心的数据文件夹。请选择它存放数据的文件夹（它的 LODESTONE_PATH）。在 Docker 中运行的核心需要从这台电脑挂载数据文件夹。"
        }
        M::ClaimNotThisCore => "应答的核心并不是找到的文件夹所属的核心，因此没有保存任何内容。",
        M::ClaimNoResult => "核心没有完成认领。请重试。",
        M::ClaimNoCertificate => {
            "此核心没有 HTTPS 证书，因此 Lodestone Launcher 无法确保它的所有者密钥只会发送给它。请在核心的设置中打开 HTTPS，然后重新连接。"
        }
        M::ClaimFiles => "无法使用核心的数据文件夹：{error}",
        M::ClaimInvalid => "核心发送了 Lodestone Launcher 无法理解的内容：{error}",
        M::AddServer => "添加服务器",
        M::Dismiss => "关闭",
        M::NoServersYet => "还没有服务器",
        M::NoServersHint => {
            "按地址添加一个 Lodestone 核心。不登录时，你能看到其所有者公开列出的服务器。"
        }

        M::BadgeTrusted => "已信任",
        M::BadgePinned => "证书已固定",
        M::SignedInAs => "已以 {username} 身份登录",
        M::SignedIn => "已登录",
        M::NotSignedIn => "未登录：仅显示公开服务器",
        M::TrustedForMods => "信任此核心的模组",
        M::Offline => "离线：无法连接到此核心。请检查它是否正在运行，并且与你处于同一网络中。",
        M::NotListedPublicly => {
            "所有者尚未在核心设置中开启“公开列出服务器”，因此不登录就没有可显示的内容。"
        }
        M::NoPublicServers => "没有公开列出的服务器。登录后可查看你有权访问的服务器。",
        M::NoVisibleServers => "此核心上没有你能看到的服务器。",
        M::CertChangedWarning => {
            "此核心的 HTTPS 证书在你信任之后已更改，因此 Lodestone Launcher 拒绝连接。核心会在内置证书即将过期或地址更改时生成新证书；这也可能意味着有人正在拦截连接。信任新证书前请先核对其指纹。"
        }
        M::CertUntrustedWarning => {
            "{computer} 不信任此核心的 HTTPS 证书。Lodestone 核心会自行生成证书，因此这是正常的：请核对其指纹，然后信任它。"
        }
        M::ReviewNewCertificate => "信任新证书…",
        M::ReviewCertificate => "信任证书…",

        M::Details => "详情",
        M::CreateInstance => "创建实例",
        M::StartAndJoin => "启动并加入",
        M::Play => "开始游戏",
        M::More => "更多…",
        M::InstanceReady => "实例已就绪",
        M::AddressHidden => "所有者未共享此服务器的地址。",

        M::StateStarting => "正在启动",
        M::StateRunning => "运行中",
        M::StateStopping => "正在停止",
        M::StateStopped => "已停止",
        M::StateCrashed => "已崩溃",
        M::StateUnknown => "未知",
        M::StateHidden => "状态已隐藏",
        M::LoaderOther => "其他",
        M::PlayersOfMax => "{count}/{max} 名玩家",

        M::BusyFetching => "正在获取服务器的模组…",
        M::BusyCreating => "正在创建实例并下载…",
        M::BusyCreatingFiles => "正在创建实例，已下载 {done}/{total} 个文件…",
        M::BusyChecking => "正在检查服务器的模组…",
        M::BusyUpdating => "正在更新模组…",
        M::BusyUpdatingFiles => "正在更新模组，{done}/{total} 个文件…",
        M::BusyStarting => "正在启动服务器…",
        M::BusyStopping => "正在停止…",

        M::ReadFailed => "无法读取你的服务器",
        M::SaveFailed => "无法保存你的服务器",
        M::NoLongerTrusted => "已不再信任 {name} 的模组",
        M::NoLongerTrustedIdChanged => {
            "现在由另一个核心在 {address} 响应（其 ID 已更改）。只有在你信任其所有者时，才在它的卡片上重新信任它。"
        }
        M::NoLongerTrustedOtherCore => "现在由另一个核心在 {address} 响应。",
        M::NoLongerTrustedNewCertificate => {
            "你为它信任了新证书。如果确定它是同一个核心，请在它的卡片上重新信任其模组。"
        }
        M::NoLongerTrustedCertificateChanged => {
            "它的证书已更改。如果确定它是同一个核心，请在它的卡片上重新信任其模组。"
        }
        M::SignedOutOf => "已从 {name} 退出登录",
        M::KeyStillValid => {
            "此设备的密钥（lsk_{id}_…）在过期前仍然有效。要立即使其失效，请在控制面板的“设置 → API 密钥”中吊销它。"
        }
        M::KeySaveFailed => "无法将密钥保存到{keychain}",
        M::KeySaveFailedError => "无法将密钥保存到{keychain}：{error}",
        M::CheckCertificate => "请核对核心的证书以继续。",
        M::CertificateNotTrusted => "证书未被信任，因此你的登录信息没有发送给核心。",
        M::CreateFailed => "无法创建实例",
        M::CreatedWithoutMods => "已创建 {name}，但不含服务器的模组",
        M::CreatedWithoutModsBody => {
            "所有者未共享此服务器的客户端模组，因此该实例只有其 Minecraft 版本和加载器。请自行添加它需要的模组。"
        }
        M::LinkFailed => "无法将实例关联到服务器",
        M::ReadyTitle => "{name} 已就绪",
        M::ReadyBody => {
            "Minecraft {version}，使用 {loader}，并带有服务器的模组和配置文件。点击“开始游戏”即可加入。"
        }
        M::CheckModsFailed => "无法检查服务器的模组",
        M::JoinFailed => "无法加入 {name}",
        M::CoreRemoved => {
            "创建此实例所对应的核心（{address}）已不在 Lodestone Launcher 中。请在“Lodestone Host”页面重新添加它，或直接点击“开始游戏”而不加入服务器。"
        }
        M::ServerNotShown => "核心不再向你显示此服务器：它已被删除，或者它未公开列出而你没有登录。",
        M::NotRunning => "{name} 未在运行",
        M::AskOwnerToStart => "请让其所有者启动它，或在“Lodestone Host”页面登录核心后自行启动。",
        M::UpdateModsFailed => "无法更新模组",
        M::StopGameFirst => "请先退出游戏。",
        M::UpdateInstanceFailed => "无法更新实例",
        M::InstanceNotSaved => "模组已更新，但无法保存实例：{error}",
        M::BadJoinAddress => "Minecraft 无法加入此服务器的地址或端口。",
        M::StartCrashed => "服务器在启动时崩溃了。请在控制面板中查看它的控制台。",
        M::StartStopped => "服务器在启动时停止了。请在控制面板中查看它的控制台。",
        M::ServerGone => "该服务器已不存在。",
        M::StartTimedOut => "服务器未能在 5 分钟内完成启动。",
        M::StartFailed => "无法启动 {name}",
        M::StopFailed => "无法停止 {name}",
        M::UnlinkFailed => "无法取消关联实例：{error}",
        M::Cancelled => "已取消",
        M::TrustExplanation => {
            "来自 Lodestone 核心的模组会作为程序在{computer} 上运行，拥有与你相同的文件和账户访问权限。只在你信任其所有者时才信任该核心。"
        }
        M::FingerprintHelp => {
            "请将它与核心显示的指纹进行比对：在控制面板的“设置 → 核心设置 → 信息”中（“证书指纹（SHA-256）”），或在核心启动时的日志中（“TLS certificate SHA-256 fingerprint”）。"
        }

        M::CoreUnreachable => "无法连接到核心。请检查地址，以及核心是否正在运行。（{detail}）",
        M::CoreUntrusted => {
            "{computer} 不信任 {host} 的 HTTPS 证书。请核对其指纹，然后信任它以进行连接。"
        }
        M::CoreCertificateChanged => {
            "{host} 的 HTTPS 证书在你信任之后已更改，因此 Lodestone Launcher 拒绝连接。信任新证书前请先核对其指纹。"
        }
        M::CoreSignedOut => "已退出登录：此设备的密钥已被吊销或已过期。请重新登录。",
        M::CoreBadCredentials => "用户名或密码错误。",
        M::CoreForbidden => "核心拒绝了请求：你没有执行此操作的权限。",
        M::CoreNotFound => "未找到",
        M::CoreTooOld => {
            "此核心版本过旧，无法用于 Lodestone Launcher。请将 Lodestone Core 更新到支持服务器列表的版本。"
        }
        M::CoreTooManyRequests => "尝试次数过多。请几分钟后再试。",
        M::CoreAnswered => "核心返回了 {status}：{message}",
        M::CoreInvalidReply => "核心发送了 Lodestone Launcher 无法理解的回复：{detail}",

        M::DifferentMods => "{name} 的模组有所不同",
        M::DifferentModsBody => {
            "自此实例获取服务器的模组以来，它们已发生变化（{changes}）。请先更新才能加入；你自己的模组保持不变，被替换的文件会保留在实例的备份中。"
        }
        M::ChangeNew => "新增：{name}",
        M::ChangeChanged => "已更改：{name}",
        M::ChangeRemoved => "已移除：{name}",
        M::ChangeConfig => "配置文件：{name}",
        M::NoChanges => "没有变化",
        M::UpdateAndPlay => "更新并开始游戏",
        M::PlayAnyway => "仍要开始游戏",

        M::CertChangedTitle => "{name} 的证书已更改",
        M::CertTrustTitle => "信任此核心的证书？",
        M::CertChangedIntro => {
            "位于 {address} 的核心出示的 HTTPS 证书与你信任的不同。核心会在内置证书即将过期或地址更改时生成新证书，但这也可能意味着有人正在拦截连接。"
        }
        M::CertUntrustedIntro => {
            "{os} 不信任 {name} 在 {address} 上使用的 HTTPS 证书。Lodestone 核心会自行生成证书，因此首次连接时出现这种情况是正常的。"
        }
        M::FingerprintTrustedBefore => "此前信任的证书（SHA-256）",
        M::FingerprintPresented => "当前出示的证书（SHA-256）",
        M::FingerprintCaption => "证书指纹（SHA-256）",
        M::CertIfMatch => {
            "如果两者一致，请信任它：Lodestone Launcher 随后只会为 {host} 接受这一个证书，并在它更改时提醒你。"
        }
        M::CertIfMatchChanged => {
            "如果两者一致，请信任它：Lodestone Launcher 随后只会为 {host} 接受这一个证书，并在它更改时提醒你。已信任其模组的核心将不再被信任，直到你重新信任它。"
        }
        M::TrustNewCertificate => "信任新证书",
        M::TrustCertificate => "信任证书",

        M::TrustTitle => "你信任 {name} 吗？",
        M::CoreAt => "{name}，地址为 {address}。",
        M::TrustAfterAdd => {
            "在你信任它之前，Lodestone Launcher 会在安装服务器的模组前先向你展示它们。你之后可以通过核心卡片上的“{setting}”更改此设置。"
        }
        M::TrustLater => {
            "之后来自此核心的模组将直接安装，不再询问。你可以在核心卡片上再次关闭此设置。"
        }
        M::TrustThisCore => "信任此核心",

        M::AdvisoryTitle => "此服务器的模组将在{computer} 上运行",
        M::AdvisoryNew => {
            "{core} 上 {server} 的实例将获得这些文件。模组就是程序：它们运行时拥有与你相同的文件和账户访问权限。"
        }
        M::AdvisoryUpdate => {
            "更新到 {core} 上 {server} 的模组会安装这些文件。模组就是程序：它们运行时拥有与你相同的文件和账户访问权限。"
        }
        M::ModrinthUnreachable => "无法连接到 Modrinth，因此无法核对任何文件。",
        M::UploadedNotChecked => "上传到服务器的文件 — 未经 Modrinth 核对（{count}）",
        M::AlsoOnModrinth => "也在 Modrinth 上（{count}）",
        M::AlsoFiles => "另有 {files}。",
        M::AlsoFilesScripts => "另有 {files}。脚本文件（KubeJS、CraftTweaker）同样可以驱动代码。",
        M::OnlyIfYouTrust => "只有在你信任 {core} 的所有者时才继续。",
        M::AdvisoryTrustNote => "“{button}”还会让此后来自它的模组直接安装，不再询问。{explanation}",
        M::InstallThisTime => "仅本次安装",
        M::TrustAndInstall => "信任 {core} 并安装",

        M::KeyDays90 => "90 天",
        M::KeyYear => "1 年",
        M::KeyNever => "永不过期",
        M::AddressPlaceholder => "192.168.1.20 或 localhost:16662",
        M::UsernameOptional => "用户名（可选）",
        M::Username => "用户名",
        M::Password => "密码",
        M::EnterAddress => "请输入核心的地址。",
        M::EnterUsernamePassword => "请输入你的用户名和密码。",
        M::EnterPassword => "请输入密码，或将用户名留空。",
        M::ConsoleAccess => "控制台访问（可选）",
        M::ReadConsole => "读取控制台",
        M::SendChat => "发送聊天和命令",
        M::ConsoleAccessHint => {
            "仅在你的用户有权使用控制台的服务器上生效。命令可以执行管理员能做的任何事，因此只有在你信任所有使用{computer} 的人时才允许。"
        }
        M::SignInTitle => "登录服务器",
        M::SignInIntro => {
            "登录后即可启动和停止服务器，并查看未公开列出的服务器。Lodestone Launcher 会为{computer} 生成专用密钥；你的密码不会被保存。"
        }
        M::AddIntro => "Lodestone 核心的地址。不登录时，你能看到其所有者公开列出的服务器。",
        M::Address => "地址",
        M::SignInOptional => "登录（可选）",
        M::KeyExpiresAfter => "{computer} 上的密钥有效期",
        M::NeverExpiresWarning => "永不过期的密钥在你于控制面板中吊销之前一直有效。",

        M::ChatPlaceholder => "说点什么，或输入 /命令 来执行",
        M::ConsoleNotAllowedAtSignIn => "{computer} 登录时未允许读取控制台。",
        M::ConsoleKeyCantRead => {
            "{computer} 上的密钥无法读取控制台。如果你的用户可以使用控制台，请重新登录并允许“{option}”。"
        }
        M::ConsoleKeyCantReadShort => "{computer} 上的密钥无法读取控制台。",
        M::ConsoleKeyCantWrite => {
            "{computer} 上的密钥无法发送聊天或命令。如果你的用户可以使用控制台，请重新登录并允许“{option}”。"
        }
        M::OfflineCantLink => "离线账户无法关联：它们的 UUID 由名称生成，因此无法证明身份。",
        M::LinkedTo => "已将 {account} 关联到 {user}。",
        M::YourLodestoneUser => "你的 Lodestone 用户",
        M::Connecting => "正在连接…",
        M::Live => "实时",
        M::Reconnecting => "正在重新连接：{reason}",
        M::ConsoleNotShown => "控制台未显示",
        M::OutputAppearsHere => "服务器的输出会显示在这里。",
        M::Send => "发送",
        M::ChatHint => "文字会通过 /say 发给所有人。以 / 开头可执行命令。",
        M::Console => "控制台",
        M::JumpToLatest => "跳到最新",
        M::Players => "玩家",
        M::NoOneOnline => "没有人在线。",
        M::ServerNotRunning => "服务器未在运行。",
        M::MinecraftAccount => "Minecraft 账户",
        M::CoreTooOldToLink => "此核心版本过旧，无法关联 Minecraft 账户。",
        M::Unlink => "取消关联",
        M::LinkAccount => "将 {account} 关联到 {user}",
        M::LinkCheckHint => "核心会使用这次登录向 Microsoft 确认该账户属于你，仅此一次，不会保留。",
        M::SignInMicrosoftToLink => "使用 Microsoft 账户登录，即可将其关联到你的 Lodestone 用户。",

        M::HostTitle => "在 Lodestone 核心上托管",
        M::ShowOnServersPage => "在“Lodestone Host”页面查看",
        M::HostIntro => {
            "以 Modrinth 整合包的形式，用 {name} 的模组和配置文件创建一个服务器。来自 Modrinth 的仅客户端模组会被排除。服务器创建后处于停止状态。"
        }
        M::CoreLabel => "核心",
        M::PasswordNotKept => "你的密码只会在登录时发送一次，不会被保存。",
        M::ServerName => "服务器名称",
        M::Memory => "内存",
        M::GamePortHint => "游戏端口是核心所在机器上从 {port} 起第一个空闲的端口。",
        M::EulaAgree => "我同意 Minecraft EULA（{url}）。Lodestone Core 会为新服务器接受它。",
        M::CreateServer => "创建服务器",
        M::NameTheServer => "请为服务器命名。",
        M::HostNeedsPassword => {
            "请输入你在此核心上的用户名和密码。创建服务器需要比{computer} 上的密钥更高的权限。"
        }
        M::AgreeEulaFirst => "请先同意 Minecraft EULA：核心会为新服务器接受它。",
        M::StageExporting => "正在导出实例…",
        M::StageSigningIn => "正在登录…",
        M::StageUploading => "正在上传整合包（{size}）…",
        M::StageCreating => "正在创建服务器…",
        M::CoreGone => "该核心已不存在。",
        M::InstanceGone => "该实例已不存在。",
        M::PackNotKept => "核心没有保留该整合包。",
        M::SignedOutRightAway => "核心在你登录后立即让你退出登录。请重试。",
        M::AddCoreFirst => "请先在“Lodestone Host”页面添加你的 Lodestone 核心。",

        M::LanListenFailed => "无法监听局域网世界：{error}",
        M::LanLooking => "正在查找对局域网开放的世界…",
        M::LanNoneMac => {
            "未找到局域网世界。在 Minecraft 中打开一个世界，然后选择“对局域网开放”。如果 macOS 询问过是否允许 Lodestone Launcher 查找本地网络上的设备，请在“系统设置 → 隐私与安全性 → 本地网络”中允许，然后重新打开此页面。"
        }
        M::LanNoneWindows => {
            "未找到局域网世界。在 Minecraft 中打开一个世界，然后选择“对局域网开放”。如果 Windows 询问过是否允许 Lodestone Launcher 通过防火墙，请在专用网络上允许它，然后重新打开此页面。"
        }
        M::LanNoneOther => {
            "未找到局域网世界。在 Minecraft 中打开一个世界，然后选择“对局域网开放”，再重新打开此页面。"
        }
        M::LanWorld => "局域网世界",
        M::Join => "加入",
        M::OnYourNetwork => "你的网络",
        M::OnYourNetworkHint => "此网络中对局域网开放的世界。",
        M::JoinWorld => "加入 {world}",
        M::PickInstance => {
            "选择用来游玩的实例。它需要与位于 {address} 的世界拥有相同的 Minecraft 版本和模组。"
        }
        M::NoInstancesYet => "你还没有实例。请先创建一个。",

        M::ANewInstance => "新实例",
        M::NewMatchingInstance => "新的匹配实例",
        M::NoResourcePack => "此服务器没有设置资源包。",
        M::DownloadToInstance => "下载到实例",
        M::PackNote => "Minecraft 会在你加入时下载它；保存到 {target} 后，离线时也能使用。",
        M::PackNoteRequired => {
            "玩家必须接受它才能加入。Minecraft 会在你加入时下载它；保存到 {target} 后，离线时也能使用。"
        }
        M::PackNoteNoSha => {
            "服务器没有提供它的 SHA-1。Minecraft 会在你加入时下载它；保存到 {target} 后，离线时也能使用。"
        }
        M::PackNoteRequiredNoSha => {
            "玩家必须接受它才能加入。服务器没有提供它的 SHA-1。Minecraft 会在你加入时下载它；保存到 {target} 后，离线时也能使用。"
        }
        M::InstanceLabel => "实例",
        M::ResourcePack => "资源包",
        M::World => "世界",
        M::WorldCopyHint => {
            "服务器世界的单人游戏副本，放在一个新的世界文件夹中。你在其中做的更改只保留在{computer} 上。"
        }
        M::StopToDownload => "请先停止服务器再下载其世界：运行中的服务器可能正在写入它。",
        M::PasswordNotKeptDownload => "你的密码只会在为这次下载登录时发送一次，不会被保存。",
        M::DownloadWorld => "下载世界",
        M::WorldNeedsPassword => {
            "请输入你在此核心上的用户名和密码。下载世界需要读取服务器文件的权限，而{computer} 上的密钥没有此权限。"
        }
        M::SignInToSeePack => "登录此核心以查看服务器的资源包。",
        M::CreatingMatching => "正在创建匹配的实例",
        M::CreatingMatchingBody => {
            "匹配实例会在服务器的模组下载完成后创建。届时请再次打开服务器的“{more}”，选择它，然后重新下载。"
        }
        M::CouldNotCreateInstance => "无法创建实例。",
        M::QuitGameFirst => "请先退出该实例中的游戏。",
        M::WorldZipping => "核心正在压缩世界…",
        M::WorldDownloading => "正在下载…",
        M::WorldDownloadingOf => "正在下载：{done} / {total}",
        M::WorldDownloadingSize => "正在下载：{done}",
        M::WorldImporting => "正在导入…",
        M::WorldImportingOf => "正在导入：{done} / {total}",
        M::WorldImported => {
            "已作为世界“{folder}”导入到 {instance}。请从“单人游戏”中打开它；它是一个副本，因此更改不会同步回服务器。"
        }
        M::PackSaved => {
            "已保存到实例的资源包中，文件名为 {file}。在资源包列表中启用它，即可在其他服务器上使用。"
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Players => ["{count} 名玩家"; 2],
        P::ReadyBodySkipped => {
            ["Minecraft {version}，使用 {loader}，并带有服务器的模组和配置文件。整合包中有 {count} 个文件被略过。点击“开始游戏”即可加入。";
                2]
        }
        P::ModsAdded => ["新增 {count} 个"; 2],
        P::ModsUpdated => ["更新 {count} 个"; 2],
        P::ModsRemoved => ["移除 {count} 个"; 2],
        P::OtherFiles => ["{count} 个其他文件"; 2],
        P::ConfigFiles => ["{count} 个配置文件"; 2],
        P::ScriptFiles => ["{count} 个脚本文件"; 2],
        P::AndMore => ["还有 {count} 个"; 2],
        P::HostDone => {
            ["{name} 已位于 {core} 上，端口为 {port}。核心正在进行设置：它会安装 Fabric，并从 Modrinth 下载 {count} 个模组文件。\n\n服务器处于停止状态。准备好后，可以在“Lodestone Host”页面或控制面板中启动它；此实例已准备好加入该服务器。";
                2]
        }
        P::HostDoneRelinked => {
            ["{name} 已位于 {core} 上，端口为 {port}。核心正在进行设置：它会安装 Fabric，并从 Modrinth 下载 {count} 个模组文件。\n\n服务器处于停止状态。准备好后，可以在“Lodestone Host”页面或控制面板中启动它；此实例已准备好加入该服务器。此实例之前关联到 {server}；现在它属于新服务器。";
                2]
        }
    }
}
