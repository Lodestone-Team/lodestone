use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::CopyAddress => "复制地址",
        M::CopiedText => "已复制",
        M::SettingUpPercent => "正在设置… {percent}",
        M::StepSelectGame => "选择游戏",
        M::StepBasicSettings => "基本设置",
        M::StepServerSettings => "服务器设置",
        M::StepAutoSettings => "自动化设置",
        M::StepSelectModpack => "选择整合包",
        M::StepSelectVersion => "选择版本",
        M::CheckFields => "请确保所有字段都已正确填写",
        M::CreateServer => "创建服务器",
        M::CreateFailed => "创建服务器出错：{error}",
        M::LoadingForm => "正在加载创建表单",
        M::FormFailed => "无法加载表单：{error}",
        M::GamesFailed => "无法加载游戏列表：{error}",
        M::UnknownSetting => "此设置需要更新版本的 Lodestone Host。",
        M::EulaAgree => "我同意 {link}",
        M::EulaLink => "Minecraft EULA",
        M::EulaRequired => "只有在你同意 Minecraft EULA 后，Minecraft 服务器才能运行。",
        M::FieldName => "名称",
        M::FieldNameDefault => "我的服务器",
        M::FieldDescription => "描述",
        M::FieldPort => "端口",
        M::FieldMinRam => "最小内存",
        M::FieldMinRamDescription => "单位为 MB。",
        M::FieldMaxRam => "最大内存",
        M::FieldMaxRamDescription => "单位为 MB。",
        M::FieldAutoStart => "自动启动",
        M::FieldAutoStartDescription => "在 Lodestone Core 启动时启动。",
        M::FieldRestartOnCrash => "崩溃后重启",
        M::Optional => "可选",
        M::ValidationRequired => "“{name}”为必填项",
        M::ValidationNumber => "“{name}”必须是数字",
        M::ValidationInteger => "“{name}”必须是整数",
        M::ValidationPositive => "“{name}”必须为正数",
        M::ValidationMin => "“{name}”必须大于或等于 {min}",
        M::ValidationNameCharacters => "名称不能包含以下任何字符：/ \\ : * ? \" < > |",
        M::ValidationMax => "“{name}”必须小于或等于 {max}",
        M::ValidationOneOf => "“{name}”必须是可用选项之一",
        M::MaxRamBelowMin => "最大内存不能小于最小内存",
        M::NothingFound => "未找到任何结果",
        M::CommandLineReadOnly => "此核心已禁用代码执行，此设置为只读。",
        M::CommandLineOwnerOnly => "只有所有者可以更改此设置。",
        M::GameTypeTitle => "选择游戏",
        M::GameTypeNew => "新功能",
        M::GameTypeModpack => "Modrinth 整合包",
        M::GameTypeModpackBoth => "整合包",
        M::GameTypeModpackDescriptionBoth => {
            "从 Modrinth 或 CurseForge 整合包创建 Fabric、Forge 或 NeoForge 服务器"
        }
        M::GameTypeModpackDescription => "从 Modrinth 整合包创建 Fabric、Forge 或 NeoForge 服务器",
        M::GameTypeImport => "已有的服务器",
        M::GameTypeImportDescription => "导入你已有的 Minecraft 服务器，包括世界、设置、模组和插件",
        M::StepChooseFolder => "选择文件夹",
        M::ImportFolderTitle => "选择服务器的文件夹",
        M::ImportFolderSubtitle => {
            "包含 server.properties 和世界的文件夹。Lodestone 会复制它，原文件夹保持不变。"
        }
        M::ImportFolderPlaceholder => "文件夹路径",
        M::ImportBrowse => "浏览…",
        M::ImportChooseFolder => "请先选择服务器的文件夹",
        M::ImportStopFirst => "如果服务器正在运行，请先停止它，以便完整复制世界。",
        M::ImportReading => "正在读取文件夹",
        M::ImportReadFailed => "无法读取文件夹：{error}",
        M::ImportOtherSoftware => "Lodestone 会用使用相同插件的软件运行它。",
        M::ImportNotOffered => "Lodestone 不提供此软件。请选择用来运行它的软件。",
        M::ImportSoftwareUnknown => "Lodestone 无法判断此服务器使用的软件。请在下方选择。",
        M::FieldSoftware => "服务器软件",
        M::FieldSoftwareDescription => "Lodestone 会安装自己的副本。",
        M::FieldVersion => "Minecraft 版本",
        M::FieldVersionDescription => "服务器上次运行的版本",
        M::ImportLoadingVersions => "正在加载版本",
        M::ImportVersionsFailed => "无法加载版本：{error}",
        M::ImportNoVersions => "核心没有返回任何版本",
        M::ImportVersionUnavailable => {
            "Lodestone 没有适用于 Minecraft {version} 的 {software} 服务器。请选择其他版本。"
        }
        M::ImportChooseSoftware => "请选择服务器软件",
        M::ImportServer => "导入服务器",
        M::ImportFailed => "无法导入服务器：{error}",
        M::GameTypeImage => "从文件",
        M::GameTypeImageDescription => "用其他 Host 或 Launcher 导出的 .lodeimg 文件创建服务器",
        M::StepChooseFile => "选择文件",
        M::StepImageSummary => "文件内容",
        M::ImageFileTitle => "选择 .lodeimg 文件",
        M::ImageFileSubtitle => "在你创建服务器之前，不会创建任何内容。",
        M::ImageBrowse => "选择文件…",
        M::ImageUploading => "正在读取文件…",
        M::ImageUploadFailed => "无法打开文件：{error}",
        M::ImageSummaryTitle => "文件内容",
        M::ImageKeepOperators => "保留文件中的管理员",
        M::ImageKeepOperatorsHint => "管理员可以在服务器上运行任何命令。",
        M::ImageCreate => "创建服务器",
        M::ImageCreateFailed => "无法创建服务器：{error}",
        M::CreateFailedLater => "无法创建服务器",
        M::ImageNotLodeimg => "不是 .lodeimg 文件",
        M::GameForgeDescription => "运行 Forge 模组。",
        M::GameNeoForgeDescription => {
            "社区维护的 Forge 分支，有自己的模组，适用于 Minecraft 1.20.2 及更高版本。"
        }
        M::GameFabricDescription => "运行 Fabric 模组。",
        M::GamePaperDescription => "运行 Bukkit 和 Spigot 插件。",
        M::GameSpigotDescription => {
            "经过修改的 Minecraft 服务器软件，支持插件，提供更高的性能和更多自定义选项。"
        }
        M::GameBedrockDescription => "只有 Bedrock Edition 玩家可以加入（手机、主机、Windows）。",
        M::GameOtherDescription => "未知的 Minecraft 变体：{name}",
        M::GameGenericDescription => "未知游戏：{name}",
        M::ModpackSourceTitle => "选择整合包",
        M::ModpackSourceDescription => "仅支持 Fabric、Forge 和 NeoForge 整合包。",
        M::ModpackSearchPlaceholder => "搜索整合包，或粘贴 Modrinth 链接",
        M::ModpackSearchPlaceholderBoth => "搜索整合包，或粘贴 Modrinth 或 CurseForge 链接",
        M::ModpackUpload => "上传 .mrpack",
        M::ModpackVersionTitle => "选择版本",
        M::ModpackVersionUploaded => "已上传的整合包。",
        M::ModpackDownloading => "正在下载整合包文件以读取其内容…",
        M::ModpackVersionHint => {
            "选择一个版本以查看其内容。Lodestone 会下载该版本的整合包文件进行读取；在你点击“创建服务器”之前，不会安装任何内容。"
        }
        M::ModpackSettingsTitle => "服务器设置",
        M::ModpackLoading => "正在加载整合包",
        M::ModpackLoadingVersions => "正在加载版本",
        M::ModpackVersionsFailed => "加载版本失败：{error}",
        M::ModpackNoVersions => "此整合包没有任何版本。",
        M::ModpackSelect => "选择",
        M::ModpackSelected => "已选择",
        M::ModpackSummaryModpack => "整合包",
        M::ModpackSummaryLoader => "加载器",
        M::ModpackSummaryServerFiles => "服务器文件",
        M::ModpackUnsupported => "无法使用此整合包：{reason}",
        M::ModpackInspectFailed => "读取整合包失败：{error}",
        M::ModpackNotAModpack => "这是 {type}，不是整合包",
        M::ModpackChooseMrpack => "请选择 .mrpack 文件",
        M::ModpackNoUploadId => "核心未返回上传 ID",
        M::ModpackCurseForgeUnavailable => "此版本的 Lodestone Host 无法使用 CurseForge。",
        M::ModpackCurseForgeServerPack => {
            "该文件是整合包的服务器文件，Lodestone 无法用它创建服务器。请改为选择整合包的某个版本。"
        }
        M::ModpackCurseForgeNoFile => {
            "CurseForge 上没有该整合包的这个文件。请改为选择它的某个版本。"
        }
        M::ModpackCurseForgeReading => "正在向 CurseForge 查询整合包的文件…",
        M::ModpackWebsiteDownload => "仅限网站下载",
        M::ModpackWaitingForDownload => "正在等待下载…",
        M::ModpackCurseForgeWebsite => {
            "此版本仅在 CurseForge 网站上提供，因此已在浏览器中打开它的页面。请在那里下载：Lodestone Host 会从你的“下载”文件夹中取得该 .zip 并继续。"
        }
        M::ModpackCurseForgeChecking => {
            "正在下载仅 CurseForge 提供的文件以进行校验（{done}/{total}）…"
        }
        M::ModpackCurseForgeUploading => "正在将整合包发送到 Lodestone Core…",
        M::ModpackCurseForgeByHand => {
            "这些文件只能从 CurseForge 网站下载，因此创建的服务器不包含它们。之后服务器的“模组”标签页会请你下载它们，并在每个文件到达时自动添加。"
        }
        M::ModpackUploadFailed => "上传整合包失败：{error}",
        M::ModpackUploading => "正在上传 {name}…",
        M::SortLabel => "排序方式",
        M::SortRelevance => "相关性",
        M::SortPopularity => "热门程度",
        M::SortDownloads => "下载量",
        M::SortFollows => "关注数",
        M::SortNewest => "最新发布",
        M::SortUpdated => "最近更新",
        M::SearchOpening => "正在打开链接…",
        M::SearchLinkFailed => "无法打开链接：{error}",
        M::SearchOpenLink => "打开 {slug}",
        M::SearchFailed => "搜索失败：{error}",
        M::SearchLoading => "正在加载结果",
        M::SearchNoResults => "没有结果。",
        M::SearchPage => "第 {page} 页，共 {total} 页",
        M::PreviousPage => "上一页",
        M::NextPage => "下一页",
        M::VersionRelease => "正式版",
        M::VersionBeta => "测试版",
        M::VersionAlpha => "早期测试版",
        M::NotFound => "找不到服务器",
        M::NoName => "无名称",
        M::NoDescription => "无描述",
        M::EditName => "重命名",
        M::EditDescription => "编辑描述",
        M::NameEmpty => "请输入名称。",
        M::DetailsTitle => "详细信息",
        M::DetailsName => "名称",
        M::DetailsDescription => "描述",
        M::DetailsUuid => "UUID",
        M::DetailsGame => "游戏",
        M::DetailsMinecraftVersion => "Minecraft 版本",
        M::DetailsVersion => "版本",
        M::DetailsLoader => "加载器",
        M::DetailsPort => "端口",
        M::DetailsState => "状态",
        M::DetailsPlayers => "玩家",
        M::DetailsCreated => "创建时间",
        M::DetailsAutoStart => "自动启动",
        M::DetailsRestartOnCrash => "崩溃后重启",
        M::DetailsFolder => "文件夹",
        M::PerformanceTitle => "服务器监控",
        M::CpuLoad => "CPU 负载",
        M::MemoryUsed => "已用内存",
        M::LoadingGraph => "正在加载图表",
        M::CpuUsage => "CPU 使用率",
        M::MemoryUsage => "内存使用量",
        M::UnitPercent => "%",
        M::UnitGiB => " GiB",
        M::GraphSeconds => "{seconds} 秒",
        M::GraphSecondsAgo => "{seconds} 秒前",
        M::PlayersTitle => "玩家列表",
        M::PlayersSortByName => "名称",
        M::PlayersLinkedUser => "关联此 Minecraft 账户的 Lodestone 用户",
        M::PlayersNone => "没有在线玩家",
        M::ShareTitle => "共享",
        M::ShareFriendsHeading => "你的好友",
        M::ShareFriendsNote => {
            "无需链接。服务器运行时，他们会在 Lodestone Launcher 中你的名字下看到这个服务器。"
        }
        M::ShareFriendsNone => {
            "在 Lodestone Launcher 中添加好友（Lodestone Host 页面的“好友”），即可无需链接与他们共享。"
        }
        M::ShareLinkHeading => "或发送邀请链接",
        M::ShareOff => {
            "收到链接的人可以从 Lodestone Launcher 加入，直接连接到这台电脑，无需端口转发。"
        }
        M::ShareStart => "通过链接共享",
        M::ShareStarting => "正在启动…",
        M::ShareSharing => "共享中",
        M::ShareReconnecting => "正在重新连接…",
        M::ShareStableLink => "重启后链接仍然有效。",
        M::ShareLinkLabel => "邀请链接",
        M::ShareCopy => "复制链接",
        M::ShareCopied => "已复制邀请链接",
        M::SharePreparing => "正在获取链接…",
        M::ShareHowToJoin => {
            "服务器运行期间，任何拥有此链接的人都能加入。没有 Lodestone Launcher 的人会看到下载链接。"
        }
        M::ShareHardNetwork => "此网络的路由器难以建立直接连接，部分玩家可能无法加入。",
        M::ShareSameComputer => "本机",
        M::ShareLocalNetwork => "局域网",
        M::ShareIpv6 => "IPv6",
        M::ShareInternet => "互联网",
        M::ShareStop => "停止共享",
        M::ShareStopTitle => "停止共享？",
        M::ShareStopBody => "通过链接游戏的人都会断开连接，链接也将失效。再次共享会生成新链接。",
        M::ShareNewLink => "新链接",
        M::ShareNewLinkTitle => "生成新链接？",
        M::ShareNewLinkBody => {
            "当前链接将失效，通过它游戏的人都会断开连接。请把新链接发给仍然可以加入的人。通过好友列表共享的好友不受影响。"
        }
        M::ShareNewLinkConfirm => "生成新链接",
        M::ShareNewLinkMade => "已生成新的邀请链接",
        M::ShareRemovedFriendsNote => {
            "你已移除这些好友。如果他们有此邀请链接，在你创建新链接之前他们仍可加入。"
        }
        M::ShareRemovedFriendsDismiss => "忽略",
        M::ShareRemovedFriendsToast => {
            "如果他们有邀请链接，仍可加入这些服务器。创建新链接即可阻止他们。"
        }
        M::ShareNewLinks => "生成新链接",
        M::ShareNewLinksMade => "已生成新的邀请链接。请发给仍然可以加入的人。",
        M::ShareNewLinkFailed => "无法创建新链接：{error}",
        M::ShareShort => "分享",
        M::DetailsMinRam => "最小内存",
        M::DetailsMaxRam => "最大内存",
        M::ShareConnected => "已连接的人",
        M::ShareJoins => "累计加入",
        M::ShareRemovedFriendsHeading => "已移除的好友",
        M::ShareRemovedServersHeading => "共享的服务器",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModpackShowMore => ["显示更多（还剩 {count} 个）", "显示更多（还剩 {count} 个）"],
    }
}
