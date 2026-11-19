use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::CopyAddress => "複製位址",
        M::CopiedText => "已複製",
        M::SettingUpPercent => "正在設定… {percent}",
        M::StepSelectGame => "選擇遊戲",
        M::StepBasicSettings => "基本設定",
        M::StepServerSettings => "伺服器設定",
        M::StepAutoSettings => "自動化設定",
        M::StepSelectModpack => "選擇模組包",
        M::StepSelectVersion => "選擇版本",
        M::CheckFields => "請確認所有欄位都已正確填寫",
        M::CreateServer => "建立伺服器",
        M::CreateFailed => "建立伺服器時發生錯誤：{error}",
        M::LoadingForm => "正在載入建立表單",
        M::FormFailed => "無法載入表單：{error}",
        M::GamesFailed => "無法載入遊戲列表：{error}",
        M::UnknownSetting => "此設定需要更新版本的 Lodestone Host。",
        M::EulaAgree => "我同意 {link}",
        M::EulaLink => "Minecraft EULA",
        M::EulaRequired => "只有在你同意 Minecraft EULA 後，Minecraft 伺服器才能執行。",
        M::FieldName => "名稱",
        M::FieldNameDefault => "我的伺服器",
        M::FieldDescription => "說明",
        M::FieldPort => "連接埠",
        M::FieldMinRam => "最小記憶體",
        M::FieldMinRamDescription => "單位為 MB。",
        M::FieldMaxRam => "最大記憶體",
        M::FieldMaxRamDescription => "單位為 MB。",
        M::FieldAutoStart => "自動啟動",
        M::FieldAutoStartDescription => "在 Lodestone Core 啟動時啟動。",
        M::FieldRestartOnCrash => "當機時重新啟動",
        M::Optional => "選填",
        M::ValidationRequired => "「{name}」為必填欄位",
        M::ValidationNumber => "「{name}」必須是數字",
        M::ValidationInteger => "「{name}」必須是整數",
        M::ValidationPositive => "「{name}」必須為正數",
        M::ValidationMin => "「{name}」必須大於或等於 {min}",
        M::ValidationNameCharacters => "名稱不能包含以下任何字元：/ \\ : * ? \" < > |",
        M::ValidationMax => "「{name}」必須小於或等於 {max}",
        M::ValidationOneOf => "「{name}」必須是可用的選項之一",
        M::MaxRamBelowMin => "最大記憶體不能小於最小記憶體",
        M::NothingFound => "找不到任何結果",
        M::CommandLineReadOnly => "此核心已停用程式碼執行，此設定為唯讀。",
        M::CommandLineOwnerOnly => "只有擁有者可以變更此設定。",
        M::GameTypeTitle => "選擇遊戲",
        M::GameTypeNew => "新功能",
        M::GameTypeModpack => "Modrinth 模組包",
        M::GameTypeModpackBoth => "模組包",
        M::GameTypeModpackDescriptionBoth => {
            "從 Modrinth 或 CurseForge 模組包建立 Fabric、Forge 或 NeoForge 伺服器"
        }
        M::GameTypeModpackDescription => "從 Modrinth 模組包建立 Fabric、Forge 或 NeoForge 伺服器",
        M::GameTypeImport => "現有的伺服器",
        M::GameTypeImportDescription => "匯入你已有的 Minecraft 伺服器，包含世界、設定、模組和外掛",
        M::StepChooseFolder => "選擇資料夾",
        M::ImportFolderTitle => "選擇伺服器的資料夾",
        M::ImportFolderSubtitle => {
            "包含 server.properties 和世界的資料夾。Lodestone 會複製它，原資料夾維持不變。"
        }
        M::ImportFolderPlaceholder => "資料夾路徑",
        M::ImportBrowse => "瀏覽…",
        M::ImportChooseFolder => "請先選擇伺服器的資料夾",
        M::ImportStopFirst => "如果伺服器正在執行，請先停止它，才能完整複製世界。",
        M::ImportReading => "正在讀取資料夾",
        M::ImportReadFailed => "無法讀取資料夾：{error}",
        M::ImportOtherSoftware => "Lodestone 會用使用相同外掛的軟體執行它。",
        M::ImportNotOffered => "Lodestone 不提供此軟體。請選擇用來執行它的軟體。",
        M::ImportSoftwareUnknown => "Lodestone 無法判斷此伺服器使用的軟體。請在下方選擇。",
        M::FieldSoftware => "伺服器軟體",
        M::FieldSoftwareDescription => "Lodestone 會安裝自己的副本。",
        M::FieldVersion => "Minecraft 版本",
        M::FieldVersionDescription => "伺服器上次執行的版本",
        M::ImportLoadingVersions => "正在載入版本",
        M::ImportVersionsFailed => "無法載入版本：{error}",
        M::ImportNoVersions => "核心沒有傳回任何版本",
        M::ImportVersionUnavailable => {
            "Lodestone 沒有適用於 Minecraft {version} 的 {software} 伺服器。請選擇其他版本。"
        }
        M::ImportChooseSoftware => "請選擇伺服器軟體",
        M::ImportServer => "匯入伺服器",
        M::ImportFailed => "無法匯入伺服器：{error}",
        M::GameTypeImage => "從檔案",
        M::GameTypeImageDescription => "用其他 Host 或 Launcher 匯出的 .lodeimg 檔案建立伺服器",
        M::StepChooseFile => "選擇檔案",
        M::StepImageSummary => "檔案內容",
        M::ImageFileTitle => "選擇 .lodeimg 檔案",
        M::ImageFileSubtitle => "在你建立伺服器之前，不會建立任何內容。",
        M::ImageBrowse => "選擇檔案…",
        M::ImageUploading => "正在讀取檔案…",
        M::ImageUploadFailed => "無法開啟檔案：{error}",
        M::ImageSummaryTitle => "檔案內容",
        M::ImageKeepOperators => "保留檔案中的管理員",
        M::ImageKeepOperatorsHint => "管理員可以在伺服器上執行任何指令。",
        M::ImageCreate => "建立伺服器",
        M::ImageCreateFailed => "無法建立伺服器：{error}",
        M::CreateFailedLater => "無法建立伺服器",
        M::ImageNotLodeimg => "不是 .lodeimg 檔案",
        M::GameForgeDescription => "執行 Forge 模組。",
        M::GameNeoForgeDescription => {
            "社群維護的 Forge 分支，有自己的模組，適用於 Minecraft 1.20.2 及更新版本。"
        }
        M::GameFabricDescription => "執行 Fabric 模組。",
        M::GamePaperDescription => "執行 Bukkit 和 Spigot 外掛。",
        M::GameSpigotDescription => {
            "經過修改的 Minecraft 伺服器軟體，支援外掛程式，提供更佳的效能與更多自訂選項。"
        }
        M::GameBedrockDescription => "只有 Bedrock Edition 玩家可以加入（手機、主機、Windows）。",
        M::GameOtherDescription => "未知的 Minecraft 變體：{name}",
        M::GameGenericDescription => "未知的遊戲：{name}",
        M::ModpackSourceTitle => "選擇模組包",
        M::ModpackSourceDescription => "僅支援 Fabric、Forge 和 NeoForge 模組包。",
        M::ModpackSearchPlaceholder => "搜尋模組包，或貼上 Modrinth 連結",
        M::ModpackSearchPlaceholderBoth => "搜尋模組包，或貼上 Modrinth 或 CurseForge 連結",
        M::ModpackUpload => "上傳 .mrpack",
        M::ModpackVersionTitle => "選擇版本",
        M::ModpackVersionUploaded => "已上傳的模組包。",
        M::ModpackDownloading => "正在下載模組包檔案以讀取其內容…",
        M::ModpackVersionHint => {
            "選擇一個版本以查看其內容。Lodestone 會下載該版本的模組包檔案來讀取；在你點擊「建立伺服器」之前，不會安裝任何內容。"
        }
        M::ModpackSettingsTitle => "伺服器設定",
        M::ModpackLoading => "正在載入模組包",
        M::ModpackLoadingVersions => "正在載入版本",
        M::ModpackVersionsFailed => "無法載入版本：{error}",
        M::ModpackNoVersions => "此模組包沒有任何版本。",
        M::ModpackSelect => "選擇",
        M::ModpackSelected => "已選擇",
        M::ModpackSummaryModpack => "模組包",
        M::ModpackSummaryLoader => "載入器",
        M::ModpackSummaryServerFiles => "伺服器檔案",
        M::ModpackUnsupported => "無法使用此模組包：{reason}",
        M::ModpackInspectFailed => "無法讀取模組包：{error}",
        M::ModpackNotAModpack => "這是 {type}，不是模組包",
        M::ModpackChooseMrpack => "請選擇 .mrpack 檔案",
        M::ModpackNoUploadId => "核心未傳回上傳 ID",
        M::ModpackCurseForgeUnavailable => "此版本的 Lodestone Host 無法使用 CurseForge。",
        M::ModpackCurseForgeServerPack => {
            "該檔案是模組包的伺服器檔案，Lodestone 無法用它建立伺服器。請改為選擇模組包的某個版本。"
        }
        M::ModpackCurseForgeNoFile => {
            "CurseForge 上沒有此模組包的這個檔案。請改為選擇它的某個版本。"
        }
        M::ModpackCurseForgeReading => "正在向 CurseForge 查詢模組包的檔案…",
        M::ModpackCurseForgeChecking => {
            "正在下載僅 CurseForge 提供的檔案以進行驗證（{done}/{total}）…"
        }
        M::ModpackCurseForgeUploading => "正在將模組包傳送到 Lodestone Core…",
        M::ModpackCurseForgeByHand => {
            "這些檔案只能從 CurseForge 網站下載，因此建立的伺服器不包含它們。之後伺服器的「模組」分頁會請你下載它們，並在每個檔案到達時自動加入。"
        }
        M::ModpackUploadFailed => "無法上傳模組包：{error}",
        M::ModpackUploading => "正在上傳 {name}…",
        M::SortLabel => "排序方式",
        M::SortRelevance => "關聯性",
        M::SortPopularity => "熱門程度",
        M::SortDownloads => "下載次數",
        M::SortFollows => "追蹤數",
        M::SortNewest => "最新發布",
        M::SortUpdated => "最近更新",
        M::SearchOpening => "正在開啟連結…",
        M::SearchLinkFailed => "無法開啟連結：{error}",
        M::SearchOpenLink => "開啟 {slug}",
        M::SearchFailed => "搜尋失敗：{error}",
        M::SearchLoading => "正在載入結果",
        M::SearchNoResults => "沒有結果。",
        M::SearchPage => "第 {page} 頁，共 {total} 頁",
        M::PreviousPage => "上一頁",
        M::NextPage => "下一頁",
        M::VersionRelease => "正式版",
        M::VersionBeta => "測試版",
        M::VersionAlpha => "早期測試版",
        M::NotFound => "找不到伺服器",
        M::NoName => "無名稱",
        M::NoDescription => "無說明",
        M::EditName => "重新命名",
        M::EditDescription => "編輯描述",
        M::NameEmpty => "請輸入名稱。",
        M::DetailsTitle => "詳細資訊",
        M::DetailsName => "名稱",
        M::DetailsDescription => "說明",
        M::DetailsUuid => "UUID",
        M::DetailsGame => "遊戲",
        M::DetailsMinecraftVersion => "Minecraft 版本",
        M::DetailsVersion => "版本",
        M::DetailsLoader => "載入器",
        M::DetailsPort => "連接埠",
        M::DetailsState => "狀態",
        M::DetailsPlayers => "玩家",
        M::DetailsCreated => "建立時間",
        M::DetailsAutoStart => "自動啟動",
        M::DetailsRestartOnCrash => "當機後重新啟動",
        M::DetailsFolder => "資料夾",
        M::PerformanceTitle => "伺服器監控",
        M::CpuLoad => "CPU 負載",
        M::MemoryUsed => "已用記憶體",
        M::LoadingGraph => "正在載入圖表",
        M::CpuUsage => "CPU 使用率",
        M::MemoryUsage => "記憶體使用量",
        M::UnitPercent => "%",
        M::UnitGiB => " GiB",
        M::GraphSeconds => "{seconds} 秒",
        M::GraphSecondsAgo => "{seconds} 秒前",
        M::PlayersTitle => "玩家清單",
        M::PlayersSortByName => "名稱",
        M::PlayersLinkedUser => "連結此 Minecraft 帳戶的 Lodestone 使用者",
        M::PlayersNone => "沒有在線上的玩家",
        M::ShareTitle => "分享",
        M::ShareFriendsHeading => "你的好友",
        M::ShareFriendsNote => {
            "不需要連結。伺服器執行時，他們會在 Lodestone Launcher 中你的名字下看到這個伺服器。"
        }
        M::ShareFriendsNone => {
            "在 Lodestone Launcher 中新增好友（Lodestone Host 頁面的「好友」），即可不用連結與他們分享。"
        }
        M::ShareLinkHeading => "或傳送邀請連結",
        M::ShareOff => {
            "收到連結的人可以從 Lodestone Launcher 加入，直接連線到這台電腦，無需連接埠轉送。"
        }
        M::ShareStart => "透過連結分享",
        M::ShareStarting => "正在啟動…",
        M::ShareSharing => "分享中",
        M::ShareReconnecting => "正在重新連線…",
        M::ShareStableLink => "重新啟動後連結仍然有效。",
        M::ShareLinkLabel => "邀請連結",
        M::ShareCopy => "複製連結",
        M::ShareCopied => "已複製邀請連結",
        M::SharePreparing => "正在取得連結…",
        M::ShareHowToJoin => {
            "伺服器執行期間，任何擁有此連結的人都能加入。沒有 Lodestone Launcher 的人會看到下載連結。"
        }
        M::ShareHardNetwork => "此網路的路由器難以建立直接連線，部分玩家可能無法加入。",
        M::ShareSameComputer => "本機",
        M::ShareLocalNetwork => "區域網路",
        M::ShareIpv6 => "IPv6",
        M::ShareInternet => "網際網路",
        M::ShareStop => "停止分享",
        M::ShareStopTitle => "停止分享？",
        M::ShareStopBody => "透過連結遊戲的人都會中斷連線，連結也將失效。再次分享會產生新連結。",
        M::ShareNewLink => "新連結",
        M::ShareNewLinkTitle => "產生新連結？",
        M::ShareNewLinkBody => {
            "目前的連結將失效，透過它遊戲的人都會中斷連線。請把新連結傳給仍可加入的人。透過好友名單分享的好友不受影響。"
        }
        M::ShareNewLinkConfirm => "產生新連結",
        M::ShareNewLinkMade => "已產生新的邀請連結",
        M::ShareRemovedFriendsNote => {
            "你已移除這些好友。如果他們有此邀請連結，在你建立新連結之前他們仍可加入。"
        }
        M::ShareRemovedFriendsDismiss => "忽略",
        M::ShareRemovedFriendsToast => {
            "如果他們有邀請連結，仍可加入這些伺服器。建立新連結即可阻止他們。"
        }
        M::ShareNewLinks => "產生新連結",
        M::ShareNewLinksMade => "已產生新的邀請連結。請傳給仍可加入的人。",
        M::ShareNewLinkFailed => "無法建立新連結：{error}",
        M::ShareShort => "分享",
        M::DetailsMinRam => "最小記憶體",
        M::DetailsMaxRam => "最大記憶體",
        M::ShareConnected => "已連線的人",
        M::ShareJoins => "累計加入",
        M::ShareRemovedFriendsHeading => "已移除的好友",
        M::ShareRemovedServersHeading => "分享的伺服器",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModpackShowMore => ["顯示更多（還有 {count} 個）", "顯示更多（還有 {count} 個）"],
    }
}
