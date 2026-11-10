use crate::i18n::{InstanceMsg as M, InstancePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Automation => "自动化",
        M::AutomationLead => "Lodestone 何时自动启动此服务器。",
        M::AutoStart => "自动启动",
        M::AutoStartHint => "Lodestone Core 启动时启动此服务器",
        M::RestartOnCrash => "崩溃后重启",
        M::RestartOnCrashHint => "此服务器崩溃时重新启动它",
        M::NotAvailable => "暂时不可用",
        M::NotAvailableHint => "请至少启动此服务器一次",
        M::NoPermission => "没有权限",
        M::SettingsFailed => "无法加载设置：{error}",
        M::DangerZone => "危险区域",
        M::DangerZoneLead => "这些设置可能对你的服务器造成不可逆的损害。",
        M::DeleteServer => "删除服务器",
        M::DeleteServerHint => "永久删除此服务器及其数据",
        M::DeleteTitle => "删除“{name}”",
        M::DeleteConfirm => "我明白，删除此服务器",
        M::DeleteCannotUndo => "此操作无法撤销。",
        M::DeleteWarning => "此实例的设置、世界和备份将被永久删除。继续之前，请备份所有重要数据。",
        M::Deleting => "正在删除 {name}…",
        M::OwnerOnlyAtom => "只有所有者可以控制 atom 服务器。",
        M::CommandLineReadOnly => "此核心已禁用代码执行，此设置为只读。",
        M::CommandLineOwnerOnly => "只有所有者可以更改此设置。",
        M::CannotBeEmpty => "不能为空",
        M::MustBeNumber => "必须是数字",
        M::MustBeGreaterThan => "必须大于 {min}",
        M::MustBeLessThan => "必须小于 {max}",
        M::ShowPassword => "显示",
        M::HidePassword => "隐藏",
        M::Revert => "还原",
        M::ResourcePack => "资源包",
        M::ResourcePackLead => "玩家加入时会被要求下载的资源包。",
        M::PackUrl => "URL",
        M::PackUrlHint => "资源包 zip 的直接链接。留空则不使用资源包。",
        M::PackSha1 => "SHA-1",
        M::PackSha1Hint => "让玩家可以校验下载内容并缓存资源包。",
        M::PackSha1Error => "SHA-1 应为 40 个十六进制字符。",
        M::PackCompute => "计算",
        M::PackDownloaded => "已下载 {size}。",
        M::PackRequired => "强制使用资源包",
        M::PackRequiredHint => "拒绝资源包的玩家会被断开连接。",
        M::PackPrompt => "提示信息",
        M::PackPromptHint => "询问玩家时显示的消息。纯文本或 JSON 文本组件。",
        M::PackRemove => "移除资源包",
        M::PackSaved => "资源包已保存",
        M::PackSavedRestart => "资源包已保存。重启服务器后生效。",
        M::PackUrlSpaces => "URL 不能包含空格。",
        M::PackNotAUrl => "这不是有效的 URL。",
        M::PackUrlScheme => "URL 必须以 http:// 或 https:// 开头。",
        M::Eula => "Minecraft EULA",
        M::EulaLead => "只有在你同意 Minecraft EULA 后，Minecraft 服务器才能运行。",
        M::EulaAccepted => "已接受",
        M::EulaAcceptedHint => "你已为此服务器同意 Minecraft EULA。",
        M::EulaNotAccepted => "未接受",
        M::EulaNotAcceptedHint => "你同意 Minecraft EULA 之前，此服务器不会启动。",
        M::EulaRead => "阅读 EULA",
        M::EulaAgree => "我同意 Minecraft EULA",
        M::EulaAcceptFailed => "无法接受 Minecraft EULA：{error}",
        M::EulaNoPermission => "你需要启动此服务器的权限才能接受。",
        M::EulaDialogTitle => "接受 Minecraft EULA",
        M::EulaDialogBody => "你同意 Minecraft EULA 后，{name} 才能启动。你同意吗？",
        M::EulaAcceptAndStart => "我同意，启动服务器",
        M::EventLogsLead => "此服务器发生的事件，最新的在前。核心会保留最近的事件。",
        M::EventLogsEmpty => "还没有任何事件",
        M::EventLogsFailed => "无法加载事件：{error}",
        M::EventState => "服务器现在为{state}",
        M::EventJoined => "{names} 加入了",
        M::EventLeft => "{names} 离开了",
        M::EventBy => "由 {user}",
        M::ModsUnsupported => {
            "此服务器不支持模组或插件。Fabric、Forge、Paper 和 Spigot 服务器支持。"
        }
        M::ModsNoRead => "你没有读取此服务器文件的权限。",
        M::ModsNoWrite => "你没有更改此服务器文件的权限",
        M::ModsInstalledView => "已安装",
        M::ModsBrowseView => "浏览",
        M::ModsRestartMod => "重启服务器以应用模组更改",
        M::ModsRestartPlugin => "重启服务器以应用插件更改",
        M::ModsSearchMod => "在 Modrinth 上搜索模组，或粘贴 Modrinth 链接",
        M::ModsSearchPlugin => "在 Modrinth 上搜索插件，或粘贴 Modrinth 链接",
        M::ModsSearchLabel => "搜索 Modrinth",
        M::ModsOnlyCompatible => "仅显示适用于 {gameVersion} / {loader} 的版本",
        M::ModsPlayersNeedToo => "玩家也需要安装",
        M::ModsServerOnly => "仅服务器",
        M::ModsPlayersNeed => "玩家需要安装",
        M::ModsNeededBy => "{mods} 需要它",
        M::ModsUnknown => "未知",
        M::ModsUnknownModrinth => "Modrinth 未说明玩家是否需要它，请查看该模组的页面。",
        M::ModsUnknownNotOnModrinth => "不在 Modrinth 上，请查看该模组的页面。",
        M::ModsRelease => "正式版",
        M::ModsBeta => "测试版",
        M::ModsAlpha => "早期测试版",
        M::ModsResultInstalled => "已安装：{mods}",
        M::ModsResultRemoved => "已移除：{files}",
        M::ModsResultSkipped => "已跳过：{projects}",
        M::ModsResultPlayersNeed => "玩家也需要安装 {mods}",
        M::ModsByAuthor => "作者：{author}",
        M::ModsInstalledTag => "已安装",
        M::ModsSortBy => "排序方式",
        M::ModsSortRelevance => "相关性",
        M::ModsSortDownloads => "下载量",
        M::ModsSortFollows => "关注数",
        M::ModsSortNewest => "最新发布",
        M::ModsSortUpdated => "最近更新",
        M::ModsOpening => "正在打开链接…",
        M::ModsLinkFailed => "无法打开链接：{error}",
        M::ModsOpenLink => "打开 {slug}",
        M::ModsSearchFailed => "搜索失败：{error}",
        M::ModsNoResults => "没有结果。",
        M::ModsPage => "第 {page} 页，共 {total} 页",
        M::ModsPreviousPage => "上一页",
        M::ModsNextPage => "下一页",
        M::ModsEnableFailed => "无法启用 {name}：{error}",
        M::ModsDisableFailed => "无法禁用 {name}：{error}",
        M::ModsUpdated => "已将 {name} 更新到 {version}",
        M::ModsUpdateFailed => "无法更新 {name}：{error}",
        M::ModsUpdateTo => "更新到 {version}",
        M::ModsViewOnModrinth => "在 Modrinth 上查看",
        M::ModsDeleteLabel => "删除 {name}",
        M::ModsUploaded => "已上传 {names}",
        M::ModsUploadFailed => "上传失败：{error}",
        M::ModsOnlyJars => "只能上传 .jar 文件：{names}",
        M::ModsUpdatesInstalled => "已安装更新",
        M::ModsSomeUpdatesFailed => "部分更新失败：{failures}",
        M::ModsDeleted => "已删除 {name}",
        M::ModsDeleteFailed => "删除失败：{error}",
        M::ModsDeleteTitleMod => "删除模组？",
        M::ModsDeleteTitlePlugin => "删除插件？",
        M::ModsDeleteBody => "{name}（{fileName}）将从服务器中删除。此操作无法撤销。",
        M::ModsOverwriteTitle => "覆盖现有文件？",
        M::ModsOverwrite => "覆盖",
        M::ModsOverwriteBody => "{message}要用上传的文件替换它吗？",
        M::ModsOverwriteDefault => "已安装同名文件。",
        M::ModsUpdateAll => "全部更新（{count}）",
        M::ModsCheckUpdates => "检查更新",
        M::ModsUpload => "上传",
        M::ModsUploadPrompt => "上传",
        M::ModsLoadFailedMod => "无法加载模组：{error}",
        M::ModsLoadFailedPlugin => "无法加载插件：{error}",
        M::ModsEmptyMod => "尚未安装任何模组。浏览 Modrinth 或上传 .jar 文件。",
        M::ModsEmptyPlugin => "尚未安装任何插件。浏览 Modrinth 或上传 .jar 文件。",
        M::ModsLoader => "Fabric Loader {version}",
        M::ModsLoaderNoVersion => "Fabric Loader",
        M::ModsPlainFabric => "玩家只需原版 Fabric 即可加入（Minecraft {gameVersion}，{loader}）。",
        M::ModsDownloadPack => "下载客户端整合包（.mrpack）",
        M::ModsDownloadPackFailed => "无法下载客户端整合包：{error}",
        M::ModsPackSaved => "已保存 {name}",
        M::ModsSavePack => "保存",
        M::ModsImportHint => {
            "将其导入 Prism Launcher、Modrinth App 或 ATLauncher，即可获得所有正确版本的内容。"
        }
        M::ModsProjectFailed => "无法加载项目：{error}",
        M::ModsSource => "源代码",
        M::ModsIssues => "问题反馈",
        M::ModsWiki => "Wiki",
        M::ModsServerSide => "服务器：{value}",
        M::ModsSideRequired => "必需",
        M::ModsSideOptional => "可选",
        M::ModsSideUnsupported => "不支持",
        M::ModsSideUnknown => "未知",
        M::ModsCreateFromModpack => "用此整合包创建服务器",
        M::ModsModpackNoPermission => "这是一个整合包。你没有创建服务器的权限。",
        M::ModsNoServerSupport => "此项目声明它无法在服务器上运行。",
        M::ModsWrongTypeMod => "这是一个 {type} 项目。Lodestone 在这里只安装模组。",
        M::ModsWrongTypePlugin => "这是一个 {type} 项目。Lodestone 在这里只安装插件。",
        M::ModsDescription => "描述",
        M::ModsRestOnModrinth => "其余说明请在 Modrinth 上查看。",
        M::ModsVersions => "版本",
        M::ModsWithDependencies => "同时安装必需的依赖项",
        M::ModsShowAll => "显示所有版本",
        M::ModsVersionsFailed => "无法加载版本：{error}",
        M::ModsNoVersions => "没有适用于 {loader} {gameVersion} 的版本。",
        M::ModsNoVersionsShowAll => {
            "没有适用于 {loader} {gameVersion} 的版本。打开“显示所有版本”以查看其他版本。"
        }
        M::ModsNotFor => "不适用于 {loader} {gameVersion}",
        M::ModsInstalledVersion => "已安装 {title} {version}",
        M::ModsInstallFailed => "无法安装 {title}：{error}",
        M::ModsIncompatibleTitle => "安装不兼容的版本？",
        M::ModsInstallAnyway => "仍然安装",
        M::ModsIncompatibleBody => {
            "{title} {version} 适用于 {loaders} {gameVersions}，而不是 {loader} {gameVersion}。它可能导致服务器崩溃。"
        }
        M::FriendsTitle => "好友",
        M::FriendsLead => {
            "你在 Lodestone Launcher 中添加的好友可以看到你与他们共享的服务器，无需链接即可加入。在你勾选好友之前，不会共享任何内容。"
        }
        M::FriendsAddInLauncher => "在 Lodestone Launcher 中添加好友",
        M::FriendsAddInLauncherHint => {
            "你的好友列表由这台电脑上的 Lodestone Launcher 保存。在那里添加好友后，即可在这里与他们共享此服务器。"
        }
        M::FriendsLoading => "正在加载好友…",
        M::FriendsShareWith => "与 {name} 共享此服务器",
        M::FriendsNotVerified => "未验证",
        M::FriendsMinecraft => {
            "Minecraft：{names}。如果服务器开启了白名单，共享时会把他们加入白名单。"
        }
        M::FriendsWhitelisted => "已将 {names} 加入服务器白名单",
        M::FriendsWhitelistFailed => "已与 {name} 共享，但无法把对方加入服务器白名单：{error}",
        M::FriendsConnected => "已连接",
        M::FriendsSyncFailed => "无法将好友列表同步到此核心：{error}",
        M::FriendsOpenFailed => "无法打开你的好友列表：{error}",
        M::FriendsPublishFailed => "无法通知 {name} 此核心：{error}",
        M::FriendsPublishFailedAll => "无法通知好友此核心：{error}",
        M::FriendsRetry => "重试",
        M::FriendsRetrying => "Lodestone Host 每分钟会自动重试。",
        M::FriendsSynced => "好友列表已同步",
        M::FriendsRetired => "你的好友身份已停用",
        M::FriendsRetiredHint => {
            "此核心不再与任何人共享。要重新开始，请在 Lodestone Launcher 的好友部分选择一个新名字，然后重新添加好友。"
        }
        M::FriendsOff => "此测试副本已关闭与好友共享",
        M::FriendsOffHint => {
            "它的核心或好友列表在测试文件夹中，而另一个不在，因此你真正的好友永远不会进入测试核心。要测试共享，请将 LODESTONE_PATH 和 LODESTONE_LAUNCHER_DATA_DIR 都设为测试文件夹。"
        }
        M::FriendsConfirmUnverified => {
            "你还没有和 {name} 核对安全码，所以无法确定真的是对方：别人也可能回应了你的好友链接。请先在 Lodestone Launcher 中核对安全码，或仍然共享。"
        }
        M::FriendsShareAnyway => "仍然共享",
        M::FriendsCertificate => "此核心的好友证书",
        M::FriendsCertificateHint => {
            "好友通过它识别此核心。如果这台电脑的数据可能被复制，请停用它：好友会放弃它，核心会获得一个新证书，好友改用新证书找到它。"
        }
        M::FriendsCertificateRetire => "停用证书…",
        M::FriendsCertificateRetireBody => {
            "好友会放弃此核心当前的证书，并不再信任其服务器的模组。核心会获得新证书，好友下次检查时会通过新证书找到它。现在连接的所有人都会断开。"
        }
        M::FriendsCertificateRetireConfirm => "停用并获取新证书",
        M::FriendsCertificateRetired => "此核心的好友证书已停用",
        M::FriendsCertificateRetiredHint => {
            "好友现在看不到此核心。为它获取新证书，即可再次从这台电脑共享。"
        }
        M::FriendsCertificateNew => "获取新证书",
        M::FriendsCertificateManual => {
            "好友现在看不到此核心。此核心无法自行生成新证书：请退出 Lodestone Host，删除 {path}，然后重新打开。"
        }
        M::FriendsCancel => "取消",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModsDownloads => ["{count} 次下载", "{count} 次下载"],
        P::ModsFollowers => ["{count} 位关注者", "{count} 位关注者"],
        P::ModsCountMod => [
            "{directory}/ 中有 {count} 个模组",
            "{directory}/ 中有 {count} 个模组",
        ],
        P::ModsCountPlugin => [
            "{directory}/ 中有 {count} 个插件",
            "{directory}/ 中有 {count} 个插件",
        ],
        P::ModsPackTitle => [
            "玩家需要 {count} 个模组才能加入",
            "玩家需要 {count} 个模组才能加入",
        ],
        P::ModsImportHintJars => [
            "将其导入 Prism Launcher、Modrinth App 或 ATLauncher，即可获得所有正确版本的内容。有 {count} 个模组不在 Modrinth 上，因此其文件已包含在整合包中。",
            "将其导入 Prism Launcher、Modrinth App 或 ATLauncher，即可获得所有正确版本的内容。有 {count} 个模组不在 Modrinth 上，因此其文件已包含在整合包中。",
        ],
        P::ModsShowMore => ["显示更多（还有 {count} 个）", "显示更多（还有 {count} 个）"],
        P::ModsMoreGameVersions => ["还有 {count} 个", "还有 {count} 个"],
        P::FriendsConnectedCount => ["{count} 位好友已连接", "{count} 位好友已连接"],
    }
}
