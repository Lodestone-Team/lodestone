use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::FriendsPageTitle => "好友",
        M::OnThisComputer => "这台电脑上",
        M::Rescan => "重新扫描",
        M::ScanningLocal => "正在查找这台电脑上的主机…",
        M::NoLocalCores => "这台电脑上没有正在运行的主机。",
        M::HostOfferTitle => "架设任意服务器，马上和朋友一起玩",
        M::HostOfferBody => "无需设置，无需端口转发。",
        M::HostOfferDownload => "下载 Lodestone Host",
        M::HostOfferCoreOnly => "仅下载服务器程序",
        M::ConnectAsOwner => "以所有者身份连接",
        M::ChooseDataFolder => "选择文件夹…",
        M::ChooseDataFolderPrompt => "选择",
        M::ProductHostHint => "桌面应用，控制面板在窗口中",
        M::ProductCoreHint => "独立的服务器程序，或 Docker",
        M::ConnectedAsOwner => "已以所有者身份连接",
        M::ConnectedMadeOwner => {
            "它还没有所有者，所以 Lodestone Launcher 为你创建了一个所有者账户。"
        }
        M::ConnectedOwnerKey => "此启动器现在拥有它的所有者密钥。你可以在主机的控制面板中撤销它。",
        M::OwnerAccount => "所有者账户",
        M::StartedBy => "启动方式",
        M::StartedDirectly => "直接启动",
        M::DirectNote => "此主机是直接启动的。它应通过 lodestone_cli 运行，由其安装并保持更新。",
        M::GetLodestoneCli => "获取 lodestone_cli",
        M::CredentialsReadFailed => "无法读取这台电脑上主机的所有者密钥",
        M::CredentialsSaveFailed => "无法保存所有者密钥",
        M::ClaimNoCore => "这台电脑上没有正在运行的主机。",
        M::ClaimRefused => {
            "主机拒绝授予所有者权限。请在它的设置中打开“允许这台电脑上的应用获取所有者权限”，或更新它。"
        }
        M::ClaimFolderNotFound => {
            "找不到此主机的数据文件夹。请选择它存放数据的文件夹（它的 LODESTONE_PATH）。在 Docker 中运行的主机需要从这台电脑挂载数据文件夹。"
        }
        M::ClaimNotThisCore => "应答的主机并不是找到的文件夹所属的主机，因此没有保存任何内容。",
        M::ClaimNoResult => "主机没有完成认领。请重试。",
        M::ClaimNoCertificate => {
            "此主机没有 HTTPS 证书，因此 Lodestone Launcher 无法确保它的所有者密钥只会发送给它。请在主机的设置中打开 HTTPS，然后重新连接。"
        }
        M::ClaimFiles => "无法使用主机的数据文件夹：{error}",
        M::ClaimInvalid => "主机发送了 Lodestone Launcher 无法理解的内容：{error}",
        M::AddServer => "添加主机",
        M::Dismiss => "关闭",
        M::OtherCoresTitle => "其他主机和服务器",
        M::OtherCoresHint => "你通过地址添加的其他电脑上的主机，以及你通过邀请链接加入的服务器。",
        M::NoOtherCores => "还没有其他主机或服务器",
        M::NoServersHint => "通过地址添加主机，或用别人发给你的邀请链接添加服务器。",

        M::CoreMoreActions => "{name} 的更多操作",
        M::CopyOwnerApiKey => "复制所有者 API 密钥",
        M::OwnerKeyCopied => "已复制所有者 API 密钥",
        M::OwnerKeyCopiedHint => "请妥善保管：拿到它的人都能以所有者身份控制这个主机。",
        M::CertificatePinned => "证书已固定",
        M::SignedInAs => "登录身份",
        M::CopyAddress => "复制地址",
        M::SignedIn => "已登录",
        M::NotSignedIn => "未登录：仅显示公开服务器",
        M::TrustedForMods => "信任此主机的模组",
        M::Offline => "离线：无法连接到此主机。请检查它是否正在运行，并且与你处于同一网络中。",
        M::NotListedPublicly => {
            "所有者尚未在核心设置中开启“公开列出服务器”，因此不登录就没有可显示的内容。"
        }
        M::NoPublicServers => "没有公开列出的服务器。登录后可查看你有权访问的服务器。",
        M::NoVisibleServers => "此主机上没有你能看到的服务器。",
        M::CertChangedWarning => {
            "此主机的 HTTPS 证书在你信任之后已更改，因此 Lodestone Launcher 拒绝连接。主机会在内置证书即将过期或地址更改时生成新证书；这也可能意味着有人正在拦截连接。信任新证书前请先核对其指纹。"
        }
        M::CertUntrustedWarning => {
            "{computer} 不信任此主机的 HTTPS 证书。主机会自行生成证书，因此这是正常的：请核对其指纹，然后信任它。"
        }
        M::ReviewNewCertificate => "信任新证书…",
        M::ReviewCertificate => "信任证书…",

        M::CreateInstance => "创建实例",
        M::StartAndJoin => "启动并加入",
        M::Play => "开始游戏并加入",
        M::SharedTag => "已共享",
        M::CopyInviteLink => "复制邀请链接",
        M::InviteLinkCopied => "已复制邀请链接",
        M::InviteLinkCopiedHint => "在服务器共享期间，拿到链接的人都能加入。",
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
        M::NoLongerTrusted => "不再信任其模组",
        M::NoLongerTrustedIdChanged => {
            "现在由另一个主机在这个地址响应（其 ID 已更改）。只有在你信任其所有者时，才在它的卡片上重新信任它。"
        }
        M::NoLongerTrustedOtherCore => "现在由另一个主机在这个地址响应。",
        M::NoLongerTrustedNewCertificate => {
            "你为它信任了新证书。如果确定它是同一个主机，请在它的卡片上重新信任其模组。"
        }
        M::NoLongerTrustedCertificateChanged => {
            "它的证书已更改。如果确定它是同一个主机，请在它的卡片上重新信任其模组。"
        }
        M::NotTrusted => "不受信任",
        M::SignedOut => "已退出登录",
        M::KeyStillValid => {
            "此设备的密钥在过期前仍然有效。要立即使其失效，请在控制面板的“设置 → API 密钥”中吊销它。"
        }
        M::KeySaveFailed => "无法将密钥保存到{keychain}",
        M::KeySaveFailedError => "无法将密钥保存到{keychain}：{error}",
        M::CheckCertificate => "请核对主机的证书以继续。",
        M::CertificateNotTrusted => "证书未被信任，因此你的登录信息没有发送给主机。",
        M::CreateFailed => "无法创建实例",
        M::CreatedWithoutMods => "已创建，但没有服务器的模组",
        M::CreatedWithoutModsBody => {
            "所有者未共享此服务器的客户端模组，因此该实例只有其 Minecraft 版本和加载器。请自行添加它需要的模组。"
        }
        M::NoServerMods => "无服务器模组",
        M::LinkFailed => "无法将实例关联到服务器",
        M::ReadyTitle => "实例已就绪",
        M::ReadyBody => "它带有服务器的模组和配置文件。点击“开始游戏”即可加入。",
        M::CheckModsFailed => "无法检查服务器的模组",
        M::JoinFailed => "无法加入",
        M::CoreRemoved => {
            "创建此实例所对应的主机已不在 Lodestone Launcher 中。请在“Lodestone Host”页面重新添加它，或直接点击“开始游戏”而不加入服务器。"
        }
        M::ServerNotShown => "主机不再向你显示此服务器：它已被删除，或者它未公开列出而你没有登录。",
        M::NotRunning => "未运行",
        M::AskOwnerToStart => "请让其所有者启动它，或在“Lodestone Host”页面登录主机后自行启动。",
        M::UpdateModsFailed => "无法更新模组",
        M::StopGameFirst => "请先退出游戏。",
        M::FilesBusy => "请等待该实例的文件处理完成，然后重试。",
        M::FinishFinderFirst => "请先完成“帮我找出来”的查找：它关闭了部分模组。",
        M::UpdateInstanceFailed => "无法更新实例",
        M::InstanceNotSaved => "模组已更新，但无法保存实例：{error}",
        M::BadJoinAddress => "Minecraft 无法加入此服务器的地址或端口。",
        M::StartCrashed => "服务器在启动时崩溃了。请在控制面板中查看它的控制台。",
        M::StartStopped => "服务器在启动时停止了。请在控制面板中查看它的控制台。",
        M::ServerGone => "该服务器已不存在。",
        M::StartTimedOut => "服务器未能在 5 分钟内完成启动。",
        M::StartFailed => "无法启动",
        M::StopFailed => "无法停止",
        M::UnlinkFailed => "无法取消关联实例：{error}",
        M::Cancelled => "已取消",
        M::TrustExplanation => {
            "来自主机的模组会作为程序在{computer} 上运行，拥有与你相同的文件和账户访问权限。只在你信任其所有者时才信任该主机。"
        }
        M::FingerprintHelp => {
            "请将它与主机显示的指纹进行比对：在控制面板的“设置 → 核心设置 → 信息”中（“证书指纹（SHA-256）”），或在主机启动时的日志中（“TLS certificate SHA-256 fingerprint”）。"
        }

        M::CoreUnreachable => "无法连接到主机。请检查地址，以及主机是否正在运行。（{detail}）",
        M::CoreUntrusted => {
            "{computer} 不信任 {host} 的 HTTPS 证书。请核对其指纹，然后信任它以进行连接。"
        }
        M::CoreCertificateChanged => {
            "{host} 的 HTTPS 证书在你信任之后已更改，因此 Lodestone Launcher 拒绝连接。信任新证书前请先核对其指纹。"
        }
        M::CoreSignedOut => "已退出登录：此设备的密钥已被吊销或已过期。请重新登录。",
        M::CoreBadCredentials => "用户名或密码错误。",
        M::CoreForbidden => "主机拒绝了请求：你没有执行此操作的权限。",
        M::CoreNotFound => "未找到",
        M::CoreTooOld => {
            "此主机版本过旧，无法用于 Lodestone Launcher。请将它更新到支持服务器列表的版本。"
        }
        M::CoreTooManyRequests => "尝试次数过多。请几分钟后再试。",
        M::CoreAnswered => "主机返回了 {status}：{message}",
        M::CoreInvalidReply => "主机发送了 Lodestone Launcher 无法理解的回复：{detail}",

        M::DifferentMods => "模组与服务器不同",
        M::DifferentModsBody => {
            "自从此实例获取服务器的模组以来，它们已经发生了变化。请先更新才能加入；你自己的模组保持不变，被替换的文件会保存在实例的备份中。"
        }
        M::ChangeNew => "新增",
        M::ChangeChanged => "已更改",
        M::ChangeRemoved => "已移除",
        M::ChangeConfig => "配置文件和其他文件",
        M::UpdateAndPlay => "更新并开始游戏",
        M::PlayAnyway => "仍要开始游戏",

        M::CertChangedTitle => "证书已更改",
        M::CertTrustTitle => "信任此主机的证书？",
        M::CertChangedIntro => {
            "这个主机提供的 HTTPS 证书与你信任的不同。主机会在内置证书即将过期或地址变更时生成新证书，但这也可能意味着有人在拦截连接。"
        }
        M::CertUntrustedIntro => {
            "{os} 不信任这个主机的 HTTPS 证书。主机会自行生成证书，所以第一次连接时出现这种情况是正常的。"
        }
        M::FingerprintTrustedBefore => "此前信任的证书（SHA-256）",
        M::FingerprintPresented => "当前出示的证书（SHA-256）",
        M::FingerprintCaption => "证书指纹（SHA-256）",
        M::CertIfMatch => {
            "如果一致，就信任它：Lodestone Launcher 随后只接受这个证书，并在它变更时提醒你。"
        }
        M::CertIfMatchChanged => {
            "如果一致，就信任它：Lodestone Launcher 随后只接受这个证书，并在它变更时提醒你。已信任其模组的主机将不再受信任，直到你再次信任它。"
        }
        M::TrustedOnlyFor => "仅信任于",
        M::TrustNewCertificate => "信任新证书",
        M::TrustCertificate => "信任证书",

        M::TrustTitle => "信任这个主机？",
        M::TrustAfterAdd => {
            "在你信任它之前，Lodestone Launcher 会在安装服务器的模组前先向你展示它们。你之后可以通过主机卡片上的“{setting}”更改此设置。"
        }
        M::TrustLater => {
            "之后来自此主机的模组将直接安装，不再询问。你可以在主机卡片上再次关闭此设置。"
        }
        M::TrustThisCore => "信任此主机",

        M::AdvisoryTitle => "此服务器的模组将在{computer} 上运行",
        M::AdvisoryNew => {
            "此服务器的实例会获得这些文件。模组是程序：它们运行时拥有与你相同的文件和账户访问权限。"
        }
        M::AdvisoryUpdate => {
            "更新到此服务器的模组会安装这些文件。模组是程序：它们运行时拥有与你相同的文件和账户访问权限。"
        }
        M::ModrinthUnreachable => "无法连接到 Modrinth，因此无法核对任何文件。",
        M::UploadedNotChecked => "上传到服务器，未经 Modrinth 检查",
        M::AlsoOnModrinth => "Modrinth 上也有",
        M::ConfigFilesKey => "配置文件",
        M::ScriptFilesKey => "脚本文件",
        M::OtherFilesKey => "其他文件",
        M::CanRunCode => "可运行代码",
        M::ScriptsNote => "脚本文件（KubeJS、CraftTweaker）也能驱动代码。",
        M::OnlyIfYouTrust => "只有在你信任主机的所有者时才继续。",
        M::AdvisoryTrustNote => "“{button}”还会让此后来自它的模组直接安装，不再询问。{explanation}",
        M::InstallThisTime => "仅本次安装",
        M::TrustAndInstall => "信任并安装",
        M::BusyPreparing => "正在等待服务器准备文件…",
        M::BusyBasePack => "正在获取服务器的整合包（{done} / {total}）…",
        M::LacksFiles => "服务器有你无法获得的文件",
        M::LacksFilesBody => {
            "你的实例已有服务器的文件，但服务器还有一些无法共享的文件。没有它们你可能无法加入。"
        }
        M::InstallTitle => "安装服务器的文件？",
        M::InstallIntro => "新实例会获得这个游戏版本和加载器，以及来自服务器的这些文件。",
        M::SummaryAddedMods => "服务器添加的模组",
        M::SummaryServerOnly => "只有服务器才有的文件",
        M::MissingTitle => "服务器无法共享的文件",
        M::MissingNote => "你将没有这些文件，缺少它们可能无法加入。请询问服务器的所有者。",
        M::WithheldSymlink => "符号链接",
        M::WithheldUnreadable => "无法读取",
        M::WithheldBadName => "名称无法共享",
        M::WithheldKeptChanging => "一直在变化",
        M::WithheldCurseForge => "CurseForge 不允许",
        M::WithheldOther => "未共享",
        M::CurseForgeTitle => "请自行从 CurseForge 下载这些",
        M::CurseForgeNote => "它们的作者不允许其他应用下载。",
        M::Install => "安装",
        M::ErrUpdateLodestone => {
            "此服务器运行的是更新版本的 Lodestone。请更新 Lodestone Launcher 后再加入。"
        }
        M::ErrPrepareFailed => "服务器无法准备其文件。请几分钟后再试。",
        M::ErrStillPreparing => "服务器仍在准备其文件。请几分钟后再试。",
        M::ErrFileFailed => "服务器无法发送 {path}。请稍后再试。",
        M::ErrViewChanged => "下载期间服务器的文件发生了变化。请重试。",
        M::ErrFilesChangedSince => {
            "{path} 在更新准备好之后发生了变化，因此没有做任何更改。请再按一次“开始游戏”。"
        }
        M::ErrBasePackGone => {
            "服务器的整合包已不在 Modrinth 上，因此无法下载其文件。请让服务器的所有者更新整合包。"
        }
        M::ErrGameChanged => {
            "服务器现在运行 {game}，而此实例适用于 {instance}。请为该服务器新建一个实例。"
        }
        M::ErrCurseForgeUnreachable => {
            "无法连接 CurseForge，而服务器有来自它的文件。请检查网络连接，然后重试。"
        }
        M::ErrCurseForgeBase => {
            "服务器基于 CurseForge 整合包，Lodestone Launcher 暂时无法从服务器安装。"
        }
        M::ErrRefused => "服务器发送了 Lodestone Launcher 不会安装的文件：{detail}",
        M::ErrBaseFileMissing => {
            "服务器的整合包中没有文件 {path}，但服务器说它来自整合包。请让服务器的所有者更新整合包。"
        }
        M::ErrOlderCore => "该主机运行的是较旧版本的 Lodestone，无法以这种方式共享其服务器的文件。",
        M::ErrInstallFiles => "无法安装服务器的文件：{detail}",
        M::ErrStoppedSharing => {
            "服务器已停止共享其文件，因此无法检查这些文件。请询问服务器的所有者。"
        }
        M::ErrNotEnoughSpace => "可用空间不足以存放服务器的文件：需要 {needed}，可用 {free}。",
        M::ErrNotLinked => "该实例已不再关联到此服务器，因此没有安装任何内容。",

        M::KeyDays90 => "90 天",
        M::KeyYear => "1 年",
        M::KeyNever => "永不过期",
        M::AddressPlaceholder => "192.168.1.20 或 localhost:16662",
        M::UsernameOptional => "用户名（可选）",
        M::Username => "用户名",
        M::Password => "密码",
        M::EnterAddress => "请输入主机的地址。",
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
        M::AddIntro => "主机的地址。不登录时，你能看到其所有者公开列出的服务器。",
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
        M::LinkedTo => "账户已关联。",
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
        M::MinecraftAccount => "Minecraft 账户",
        M::CoreTooOldToLink => "此主机版本过旧，无法关联 Minecraft 账户。",
        M::Unlink => "取消关联",
        M::LinkAccount => "关联",
        M::LodestoneUser => "Lodestone 用户",
        M::LinkCheckHint => "主机会使用这次登录向 Microsoft 确认该账户属于你，仅此一次，不会保留。",
        M::SignInMicrosoftToLink => "使用 Microsoft 账户登录，即可将其关联到你的 Lodestone 用户。",

        M::HostTitle => "在 Lodestone 上托管",
        M::ShowOnServersPage => "在“Lodestone Host”页面查看",
        M::CoreLabel => "主机",
        M::PasswordNotKept => "你的密码只会在登录时发送一次，不会被保存。",
        M::ServerName => "服务器名称",
        M::Memory => "内存",
        M::EulaAgree => "我同意 Minecraft EULA（{url}）。主机会为新服务器接受它。",
        M::ReadEula => "阅读 EULA",
        M::CreateServer => "创建服务器",
        M::NameTheServer => "请为服务器命名。",
        M::HostNeedsPassword => {
            "请输入你在此主机上的用户名和密码。创建服务器需要比{computer} 上的密钥更高的权限。"
        }
        M::AgreeEulaFirst => "请先同意 Minecraft EULA：主机会为新服务器接受它。",
        M::StageExporting => "正在导出实例…",
        M::StageCreating => "正在创建服务器…",
        M::CoreGone => "该主机已不存在。",
        M::InstanceGone => "该实例已不存在。",
        M::SignedOutRightAway => "主机在你登录后立即让你退出登录。请重试。",
        M::AddCoreFirst => "请先在“Lodestone Host”页面添加你的主机。",
        M::HostIntro => {
            "根据此实例在主机上创建服务器：包括它的模组、配置文件以及你选择的世界。仅玩家需要的模组、你的账户和你自己的设置会留在这里。服务器创建后处于停止状态。"
        }
        M::HostWorlds => "要发送的世界",
        M::HostWorldsHint => "下一步选择哪个世界成为服务器的世界。不发送世界则创建新世界。",
        M::HostNoWorlds => "此实例没有世界：服务器会创建一个新世界。",
        M::HostWorldsLoading => "正在查找实例的世界…",
        M::HostStageSending => "正在发送到主机…",
        M::Sent => "已发送",
        M::HostStageReading => "主机正在读取文件…",
        M::HostPort => "端口",
        M::HostPortInvalid => "请输入 1 到 65535 之间的端口。",
        M::HostNameForbidden => "服务器名称不能包含 / \\ : * ? \" < > |",
        M::HostNameTooLong => "服务器名称最多 100 个字符。",
        M::HostNameControl => "服务器名称不能包含控制字符。",
        M::HostCreatingHint => "你可以关闭此窗口：主机会继续设置服务器，准备好后此实例会关联到它。",
        M::HostCreated => {
            "它目前处于停止状态：准备好后，请在“Lodestone Host”页面或控制面板中启动它。此实例已关联到它，所以“开始游戏”会加入它。"
        }
        M::HostCreatedUnlinked => {
            "它目前处于停止状态：准备好后，请在“Lodestone Host”页面或控制面板中启动它。实例在此期间已被删除，所以没有任何内容关联到它。"
        }
        M::HostCreatedRelinked => "此实例之前关联到另一个服务器；现在它属于新的服务器。",
        M::LinkedBefore => "之前关联",
        M::HostLeftOut => "服务器上没有的内容",
        M::HostCreationFailed => "主机无法创建服务器：{error}",
        M::HostCreationFailedPlain => "主机无法创建服务器。",
        M::HostStillCreating => {
            "主机仍在设置服务器。准备好后它会出现在“Lodestone Host”页面上；此实例还没有关联到它。"
        }
        M::HostTooOld => {
            "该主机的 Lodestone 版本太旧，无法从文件创建服务器。请在那台电脑上更新它，然后重试。"
        }
        M::HostNoPermission => "你在这个主机上的用户无法创建服务器。请询问其所有者。",
        M::HostGameRunning => "请先退出游戏：游戏运行时无法发送它的世界。",
        M::DownloadWorldAction => "下载这个世界…",
        M::DownloadWorldTitle => "下载世界",
        M::DownloadWorldIntro => {
            "根据此服务器的一个备份，在这台电脑上创建新实例：包括它的世界以及玩家获得的模组。服务器的设置、管理员和插件仍保留在服务器上。"
        }
        M::DownloadWorldSignIn => "请用可以读取此服务器文件的用户登录。",
        M::DownloadWorldNeedsPassword => "请输入你在此主机上的用户名和密码。",
        M::DownloadWorldBackups => "备份",
        M::DownloadWorldMidWrite => "在服务器运行时备份：世界可能在写入途中被保存。",
        M::TakenWhileRunning => "运行时备份",
        M::DownloadWorldBackUpNow => "立即备份",
        M::DownloadWorldBackUpNowHint => "服务器当前状态的备份",
        M::DownloadWorldButton => "下载",
        M::DownloadWorldBackingUp => "主机正在备份服务器…",
        M::DownloadWorldPreparing => "主机正在准备文件…",
        M::DownloadWorldDownloading => "正在下载…",
        M::Downloaded => "已下载",
        M::DownloadWorldTooOld => {
            "该主机的 Lodestone 版本太旧，无法发送世界。请在那台电脑上更新它，然后重试。"
        }
        M::DownloadWorldNothingToDownload => {
            "此服务器没有备份，而你的用户无法备份它。请让它的所有者备份后再试。"
        }
        M::DownloadWorldBackupForbidden => "你的用户无法备份此服务器。请改选它的一个备份。",
        M::DownloadWorldForbidden => {
            "你的用户无法读取此服务器的文件，因此无法下载它的世界。请联系它的所有者。"
        }
        M::DownloadWorldNoBackups => "此服务器还没有备份。请先备份。",
        M::DownloadWorldBackupFailed => "备份失败：{error}",
        M::DownloadWorldCancelled => "下载已取消。",

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
        M::AddInvite => "添加",
        M::UnnamedServer => "未命名服务器",
        M::WaitingForReply => "等待其主机响应",
        M::AddAndJoin => "添加并加入",
        M::AlreadyAddedTitle => "已添加",
        M::AlreadyAddedBody => "此链接指向本页上的一个服务器，它的那一行已突出显示。",
        M::OnYourNetwork => "你的网络",
        M::OnYourNetworkHint => "此网络中对局域网开放的世界。",
        M::JoinWorld => "加入局域网世界",
        M::PickInstance => "选择用于游玩的实例。它需要与这个世界相同的 Minecraft 版本和模组。",
        M::NoInstancesYet => "你还没有实例。请先创建一个。",

        M::FriendsTitle => "来自邀请链接的服务器",
        M::FriendsHint => "别人通过 Lodestone Connect 邀请链接与你共享的服务器。",
        M::JoinAFriend => "添加服务器",
        M::SharedBy => "共享者",
        M::SharedWithFriends => "已与好友共享",
        M::SharedWithFriendsHint => "你通过它们的邀请链接加入过这些服务器。",
        M::ConnectToJoinDirectly => "以所有者身份连接后即可直接加入，无需隧道。",
        M::DirectConnection => "在这台电脑上直接连接，无需隧道",
        M::FriendCheck => "检查",
        M::FriendChecking => "正在检查…",
        M::FriendPlayingNow => "正在游戏中",
        M::StopTrusting => "不再信任",
        M::ConnectionSameComputer => "同一台电脑",
        M::ConnectionLocalNetwork => "局域网",
        M::ConnectionIpv6 => "IPv6",
        M::ConnectionInternet => "互联网",
        M::PingMs => "{ms} 毫秒",
        M::JoinFriendTitle => "加入服务器",
        M::AddServerTitle => "添加服务器",
        M::JoinFriendIntro => {
            "粘贴别人发给你的邀请链接，它以 https://connect.lodestone.cc/j/ 开头。即使服务器现在离线，“添加”也会保存它。"
        }
        M::InvitePlaceholder => "邀请链接（https://connect.lodestone.cc/j/…）",
        M::NotAnInvite => {
            "这不是 Lodestone 邀请链接。邀请链接以 https://connect.lodestone.cc/j/ 开头。"
        }
        M::PasteInviteFirst => "请先粘贴邀请链接。",
        M::ConnectingToFriend => "正在连接到好友的服务器…",
        M::StageFinding => "正在寻找你和好友网络之间的直接连接，这需要几秒钟。",
        M::StageFindingSlow => "仍在寻找。某些网络最多需要一分钟。",
        M::StageReading => "已连接。正在读取服务器信息…",
        M::FriendHasMods => "此服务器使用模组。Lodestone Launcher 会为你安装相同的模组。",
        M::FriendNoMods => "无需模组",
        M::MadeForThisServer => "为此服务器创建",
        M::SameVersion => "版本相同",
        M::MakesInstance => "点击“开始游戏”会创建一个版本相同的新实例。",
        M::MakesInstanceWithMods => "点击“开始游戏”会创建一个与服务器版本和模组相同的新实例。",
        M::FriendServerStopped => "服务器已停止",
        M::FriendServerStoppedBody => "请好友启动服务器。服务器运行后即可开始游戏。",
        M::FriendServerStarting => "服务器正在启动",
        M::FriendServerStartingBody => "服务器运行后即可开始游戏。",
        M::FriendStarting => "正在准备游戏…",
        M::FriendInstanceBusy => "该实例已在运行。请先退出游戏。",
        M::FriendLinkEnded => "好友已停止共享此服务器",
        M::FriendLinkEndedBody => "请向好友索取新链接。",
        M::FriendNotSharing => "好友目前没有共享此服务器",
        M::FriendNotSharingBody => {
            "好友的电脑或 Lodestone Host 可能已关闭。请稍后再试，或请好友开始共享。"
        }
        M::FriendNoPath => "你和好友的网络无法直接连接",
        M::FriendNoPathBody => {
            "Lodestone Connect 会直接连接两台电脑，但这两个网络阻止了这种连接。请尝试换一个网络（例如手机热点），或请好友在路由器上设置端口转发。"
        }
        M::FriendWrongHost => "这不是好友的服务器",
        M::FriendWrongHostBody => {
            "响应的电脑并不是此链接所指向的电脑，因此 Lodestone Launcher 在发送任何内容之前就停止了。可能有人冒充好友的服务器。请向好友索取新链接，并且只使用直接从好友那里得到的链接。"
        }
        M::FriendOffline => "这台电脑似乎未联网",
        M::FriendOfflineBody => "请检查网络连接，然后重试。",
        M::FriendServiceUnreachable => "无法连接到 Lodestone Connect",
        M::FriendServiceUnreachableBody => {
            "connect.lodestone.cc 没有响应（{detail}）。请检查网络连接，然后重试。"
        }
        M::FriendJoinFailed => "无法加入",
        M::FriendJoinFailedBody => "出现问题：{detail}",
        M::FriendTunnelEnded => "连接已断开",
        M::FriendTunnelEndedBody => {
            "好友已停止共享，或者其电脑已断网。Minecraft 会提示连接已丢失。等好友回来后再开始游戏。"
        }
        M::FriendLinkMissing => {
            "Lodestone Launcher 无法读取此服务器的邀请链接。请再次打开好友发来的链接。"
        }
        M::FriendRemoved => {
            "此实例是为好友的服务器创建的，但该服务器已不在 Lodestone Launcher 中。请再次打开其邀请链接以加入。"
        }
        M::InviteLinkKey => "邀请链接",
        M::FriendsReadFailed => "无法读取好友的服务器",
        M::FriendsSaveFailed => "无法保存好友的服务器",
        M::MagicLocalhost => "Magic localhost",
        M::WhileConnected => "连接期间有效。",
        M::FriendPortTaken => "端口已被占用",
        M::FriendPortTakenBody => {
            "这台电脑上的其他程序正在使用常用端口，所以这次请在 Minecraft 中使用这个地址。下次 Lodestone Launcher 会先尝试常用端口。"
        }
        M::AddressNow => "本次地址",
        M::UsualPort => "常用端口",
        M::EulaNotAccepted => "此服务器尚未接受 Minecraft EULA",
        M::EulaNotAcceptedBody => {
            "在有人接受 Mojang 的 Minecraft EULA（{url}）之前，服务器无法启动。请先阅读，只有在你同意时才接受。"
        }
        M::AcceptEula => "接受 Minecraft EULA",
        M::EulaAccepted => "你已为此服务器接受 Minecraft EULA，现在可以启动了。",
        M::EulaAcceptFailed => "无法接受 EULA：{error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Players => ["{count} 名玩家"; 2],
        P::ReadyBodySkipped => {
            ["它带有服务器的模组和配置文件。整合包中有 {count} 个文件被略过。点击“开始游戏”即可加入。";
                2]
        }
        P::ReadyBodyMissing => {
            ["它带有服务器的文件。缺少 {count} 个服务器无法共享的文件。点击“开始游戏”即可加入。"; 2]
        }
        P::AndMore => ["还有 {count} 个"; 2],
    }
}
