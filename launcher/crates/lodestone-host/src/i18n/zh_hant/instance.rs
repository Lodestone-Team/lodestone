use crate::i18n::{InstanceMsg as M, InstancePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Automation => "自動化",
        M::AutomationLead => "Lodestone 何時自動啟動此伺服器。",
        M::AutoStart => "自動啟動",
        M::AutoStartHint => "Lodestone Core 啟動時啟動此伺服器",
        M::RestartOnCrash => "當機後重新啟動",
        M::RestartOnCrashHint => "此伺服器當機時重新啟動它",
        M::NotAvailable => "暫時無法使用",
        M::NotAvailableHint => "請至少啟動此伺服器一次",
        M::NoPermission => "沒有權限",
        M::SettingsFailed => "無法載入設定：{error}",
        M::DangerZone => "危險區域",
        M::DangerZoneLead => "這些設定可能對你的伺服器造成無法復原的損害。",
        M::DeleteServer => "刪除伺服器",
        M::DeleteServerHint => "永久刪除此伺服器及其資料",
        M::DeleteTitle => "刪除「{name}」",
        M::DeleteConfirm => "我了解，刪除此伺服器",
        M::DeleteCannotUndo => "此動作無法復原。",
        M::DeleteWarning => "此實例的設定和世界將被永久刪除。繼續之前，請備份所有重要資料。",
        M::DeleteBackupsToo => "它的備份也會被刪除。",
        M::Deleting => "正在刪除 {name}…",
        M::CommandLineReadOnly => "此核心已停用程式碼執行，此設定為唯讀。",
        M::CommandLineOwnerOnly => "只有擁有者可以變更此設定。",
        M::CannotBeEmpty => "不可空白",
        M::MustBeNumber => "必須是數字",
        M::MustBeGreaterThan => "必須大於 {min}",
        M::MustBeLessThan => "必須小於 {max}",
        M::ShowPassword => "顯示",
        M::HidePassword => "隱藏",
        M::Revert => "還原",
        M::ResourcePack => "資源包",
        M::ResourcePackLead => "玩家加入時會被要求下載的資源包。",
        M::PackUrl => "URL",
        M::PackUrlHint => "資源包 zip 的直接連結。留空則不使用資源包。",
        M::PackSha1 => "SHA-1",
        M::PackSha1Hint => "讓玩家可以驗證下載內容並快取資源包。",
        M::PackSha1Error => "SHA-1 應為 40 個十六進位字元。",
        M::PackCompute => "計算",
        M::PackDownloaded => "已下載 {size}。",
        M::PackRequired => "強制使用資源包",
        M::PackRequiredHint => "拒絕資源包的玩家會被中斷連線。",
        M::PackPrompt => "提示訊息",
        M::PackPromptHint => "詢問玩家時顯示的訊息。純文字或 JSON 文字元件。",
        M::PackRemove => "移除資源包",
        M::PackSaved => "資源包已儲存",
        M::PackSavedRestart => "資源包已儲存。重新啟動伺服器後生效。",
        M::PackUrlSpaces => "URL 不可包含空格。",
        M::PackNotAUrl => "這不是有效的 URL。",
        M::PackUrlScheme => "URL 必須以 http:// 或 https:// 開頭。",
        M::Eula => "Minecraft EULA",
        M::EulaLead => "只有在你同意 Minecraft EULA 後，Minecraft 伺服器才能執行。",
        M::EulaAccepted => "已接受",
        M::EulaAcceptedHint => "你已為此伺服器同意 Minecraft EULA。",
        M::EulaNotAccepted => "未接受",
        M::EulaNotAcceptedHint => "你同意 Minecraft EULA 之前，此伺服器不會啟動。",
        M::EulaRead => "閱讀 EULA",
        M::EulaAgree => "我同意 Minecraft EULA",
        M::EulaAcceptFailed => "無法接受 Minecraft EULA：{error}",
        M::EulaNoPermission => "你需要啟動此伺服器的權限才能接受。",
        M::EulaDialogTitle => "接受 Minecraft EULA",
        M::EulaDialogBody => "你同意 Minecraft EULA 後，{name} 才能啟動。你同意嗎？",
        M::EulaAcceptAndStart => "我同意，啟動伺服器",
        M::EventLogsLead => "此伺服器發生的事件，最新的在前。核心會保留最近的事件。",
        M::EventLogsEmpty => "還沒有任何事件",
        M::EventLogsFailed => "無法載入事件：{error}",
        M::EventState => "伺服器現在為{state}",
        M::EventJoined => "{names} 加入了",
        M::EventLeft => "{names} 離開了",
        M::EventBy => "由 {user}",
        M::ModsUnsupported => {
            "此伺服器不支援模組或外掛。Fabric、Forge、Paper 和 Spigot 伺服器支援。"
        }
        M::ModsNoRead => "你沒有讀取此伺服器檔案的權限。",
        M::ModsNoWrite => "你沒有變更此伺服器檔案的權限",
        M::ModsInstalledView => "已安裝",
        M::ModsBrowseView => "瀏覽",
        M::ModsRestartMod => "重新啟動伺服器以套用模組變更",
        M::ModsRestartPlugin => "重新啟動伺服器以套用外掛變更",
        M::ModsSearchMod => "在 Modrinth 上搜尋模組，或貼上 Modrinth 連結",
        M::ModsSearchPlugin => "在 Modrinth 上搜尋外掛，或貼上 Modrinth 連結",
        M::ModsSearchLabel => "搜尋 Modrinth",
        M::ModsOnlyCompatible => "僅顯示適用於 {gameVersion} / {loader} 的版本",
        M::ModsPlayersNeedToo => "玩家也需要安裝",
        M::ModsServerOnly => "僅限伺服器",
        M::ModsPlayersNeed => "玩家需要安裝",
        M::ModsNeededBy => "{mods} 需要它",
        M::ModsUnknown => "未知",
        M::ModsUnknownModrinth => {
            "Modrinth 未說明玩家是否需要它，請查看該模組的頁面。如果玩家需要，請勾選「給玩家」。"
        }
        M::ModsUnknownNotOnModrinth => {
            "不在 Modrinth 上，請查看該模組的頁面。如果玩家需要，請勾選「給玩家」。"
        }
        M::ModsForPlayers => "給玩家",
        M::ModsForPlayersHint => {
            "玩家加入時會取得這個模組及其相依的模組。對於 Lodestone 無法判斷但玩家需要的模組（例如你上傳的），請勾選此項。"
        }
        M::ModsForPlayersAlready => "玩家已會取得這個模組：Modrinth 或模組本身表明玩家需要它。",
        M::ModsMarkedForPlayers => "玩家下次加入時會取得 {name}",
        M::ModsForPlayersOptional => {
            "Modrinth 表示玩家沒有這個模組也能加入，但在 Fabric 上，這類模組常常會讓玩家無法加入，所以玩家會取得它。取消勾選即可不給玩家。"
        }
        M::ModsUnmarkedForPlayers => "玩家加入時不再取得 {name}",
        M::ModsMarkFailed => "無法變更 {name}：{error}",
        M::ModsRelease => "正式版",
        M::ModsBeta => "測試版",
        M::ModsAlpha => "早期測試版",
        M::ModsResultInstalled => "已安裝：{mods}",
        M::ModsResultRemoved => "已移除：{files}",
        M::ModsResultSkipped => "已略過：{projects}",
        M::ModsResultPlayersNeed => "玩家也需要安裝 {mods}",
        M::ModsByAuthor => "作者：{author}",
        M::ModsInstalledTag => "已安裝",
        M::ModsSortBy => "排序方式",
        M::ModsSortRelevance => "關聯性",
        M::ModsSortDownloads => "下載次數",
        M::ModsSortFollows => "追蹤數",
        M::ModsSortNewest => "最新發布",
        M::ModsSortUpdated => "最近更新",
        M::ModsOpening => "正在開啟連結…",
        M::ModsLinkFailed => "無法開啟連結：{error}",
        M::ModsOpenLink => "開啟 {slug}",
        M::ModsSearchFailed => "搜尋失敗：{error}",
        M::ModsNoResults => "沒有結果。",
        M::ModsPage => "第 {page} 頁，共 {total} 頁",
        M::ModsPreviousPage => "上一頁",
        M::ModsNextPage => "下一頁",
        M::ModsEnableFailed => "無法啟用 {name}：{error}",
        M::ModsDisableFailed => "無法停用 {name}：{error}",
        M::ModsUpdated => "已將 {name} 更新至 {version}",
        M::ModsUpdateFailed => "無法更新 {name}：{error}",
        M::ModsUpdateTo => "更新至 {version}",
        M::ModsViewOnModrinth => "在 Modrinth 上檢視",
        M::ModsDeleteLabel => "刪除 {name}",
        M::ModsUploaded => "已上傳 {names}",
        M::ModsUploadFailed => "上傳失敗：{error}",
        M::ModsOnlyJars => "只能上傳 .jar 檔案：{names}",
        M::ModsUpdatesInstalled => "已安裝更新",
        M::ModsSomeUpdatesFailed => "部分更新失敗：{failures}",
        M::ModsDeleted => "已刪除 {name}",
        M::ModsDeleteFailed => "刪除失敗：{error}",
        M::ModsDeleteTitleMod => "刪除模組？",
        M::ModsDeleteTitlePlugin => "刪除外掛？",
        M::ModsDeleteBody => "{name}（{fileName}）將從伺服器中刪除。此動作無法復原。",
        M::ModsOverwriteTitle => "覆寫現有檔案？",
        M::ModsOverwrite => "覆寫",
        M::ModsOverwriteBody => "{message}要以上傳的檔案取代它嗎？",
        M::ModsOverwriteDefault => "已安裝同名檔案。",
        M::ModsUpdateAll => "全部更新（{count}）",
        M::ModsCheckUpdates => "檢查更新",
        M::ModsUpload => "上傳",
        M::ModsUploadPrompt => "上傳",
        M::ModsLoadFailedMod => "無法載入模組：{error}",
        M::ModsLoadFailedPlugin => "無法載入外掛：{error}",
        M::ModsEmptyMod => "尚未安裝任何模組。瀏覽 Modrinth 或上傳 .jar 檔案。",
        M::ModsEmptyPlugin => "尚未安裝任何外掛。瀏覽 Modrinth 或上傳 .jar 檔案。",
        M::ModsLoader => "Fabric Loader {version}",
        M::ModsLoaderNoVersion => "Fabric Loader",
        M::ModsPlainFabric => "玩家只需原版 Fabric 即可加入（Minecraft {gameVersion}，{loader}）。",
        M::ModsDownloadPack => "下載用戶端模組包（.mrpack）",
        M::ModsPackShowFewer => "收合",
        M::ModsDownloadPackFailed => "無法下載用戶端模組包：{error}",
        M::ModsPackSaved => "已儲存 {name}",
        M::ModsSavePack => "儲存",
        M::ModsImportHint => {
            "將其匯入 Prism Launcher、Modrinth App 或 ATLauncher，即可取得所有正確版本的內容。"
        }
        M::ModpackFrom => "此伺服器使用模組包 {pack}。",
        M::ModpackCheck => "檢查更新",
        M::ModpackCheckFailed => "無法檢查更新：{error}",
        M::ModpackUpToDate => "已是最新版本。",
        M::ModpackUpdateTo => "更新到 {version}",
        M::ModpackFromFile => "從檔案更新",
        M::ModpackFilePrompt => "更新",
        M::ModpackNotMrpack => "{name} 不是 .mrpack 檔案。",
        M::ModpackPlanFailed => "無法準備更新：{error}",
        M::ModpackDialogTitle => "更新模組包？",
        M::ModpackDialogIntro => "{pack} 將從 {from} 更新到 {to}。",
        M::ModpackDialogReinstall => "伺服器將改用 {software} 並重新安裝。",
        M::ModpackDialogKept => "你修改過的檔案將保持不變：{files}",
        M::ModpackDialogNothing => "沒有檔案會變更。",
        M::ModpackDialogStop => "請先停止伺服器：伺服器執行時無法更新。",
        M::ModpackDialogUnsupported => "無法使用此版本：{reason}",
        M::ModpackUpdate => "更新",
        M::ModpackUpdated => "已將 {pack} 更新到 {version}。",
        M::ModpackUpdateFailed => "無法更新模組包：{error}",
        M::ModpackUnknownVersion => "未知版本",
        M::ModpackWhatsNew => "更新內容",
        M::ModpackOut => "{version} 已發布",
        M::ModpackHistory => "歷史",
        M::ModpackHistoryFailed => "無法載入模組包的歷史：{error}",
        M::ModpackHistoryEmpty => "尚無記錄。之後的更新可以復原。",
        M::ModpackKindInstall => "已安裝",
        M::ModpackKindBaseline => "首次記錄",
        M::ModpackKindUpdate => "已更新",
        M::ModpackKindRestore => "已還原 {version}",
        M::ModpackCurrent => "目前",
        M::ModpackRestore => "還原",
        M::ModpackRestoreMissing => "Lodestone 已不再保有此版本的部分檔案。",
        M::ModpackRestoreTitle => "還原此版本？",
        M::ModpackRestored => "已還原 {pack} {version}。",
        M::ModpackRestoreFailed => "無法還原模組包：{error}",
        M::ModpackStoreSize => "為了復原更新，Lodestone 保留了 {size} 無法重新下載的檔案。",
        M::ModpackSince => "自 {version} 起",
        M::ModpackSinceTooltip => "此檔案隨模組包 {version} 而來。",
        M::ModsProjectFailed => "無法載入專案：{error}",
        M::ModsSource => "原始碼",
        M::ModsIssues => "問題回報",
        M::ModsWiki => "Wiki",
        M::ModsServerSide => "伺服器：{value}",
        M::ModsSideRequired => "必要",
        M::ModsSideOptional => "選用",
        M::ModsSideUnsupported => "不支援",
        M::ModsSideUnknown => "未知",
        M::ModsCreateFromModpack => "用此模組包建立伺服器",
        M::ModsModpackNoPermission => "這是一個模組包。你沒有建立伺服器的權限。",
        M::ModsNoServerSupport => "此專案表示它無法在伺服器上執行。",
        M::ModsWrongTypeMod => "這是一個 {type} 專案。Lodestone 在這裡只安裝模組。",
        M::ModsWrongTypePlugin => "這是一個 {type} 專案。Lodestone 在這裡只安裝外掛。",
        M::ModsDescription => "說明",
        M::ModsRestOnModrinth => "其餘說明請在 Modrinth 上查看。",
        M::ModsVersions => "版本",
        M::ModsWithDependencies => "同時安裝必要的相依項目",
        M::ModsShowAll => "顯示所有版本",
        M::ModsVersionsFailed => "無法載入版本：{error}",
        M::ModsNoVersions => "沒有適用於 {loader} {gameVersion} 的版本。",
        M::ModsNoVersionsShowAll => {
            "沒有適用於 {loader} {gameVersion} 的版本。開啟「顯示所有版本」以查看其他版本。"
        }
        M::ModsNotFor => "不適用於 {loader} {gameVersion}",
        M::ModsInstalledVersion => "已安裝 {title} {version}",
        M::ModsInstallFailed => "無法安裝 {title}：{error}",
        M::ModsIncompatibleTitle => "安裝不相容的版本？",
        M::ModsInstallAnyway => "仍要安裝",
        M::ModsIncompatibleBody => {
            "{title} {version} 適用於 {loaders} {gameVersions}，而非 {loader} {gameVersion}。它可能導致伺服器當機。"
        }
        M::FriendsTitle => "好友",
        M::FriendsLead => {
            "你在 Lodestone Launcher 中加入的好友可以看到你與他們分享的伺服器，不需要連結即可加入。在你勾選好友之前，不會分享任何內容。"
        }
        M::FriendsAddInLauncher => "在 Lodestone Launcher 中加入好友",
        M::FriendsAddInLauncherHint => {
            "你的好友名單由這台電腦上的 Lodestone Launcher 保存。在那裡加入好友後，即可在這裡與他們分享此伺服器。"
        }
        M::FriendsLoading => "正在載入好友…",
        M::FriendsShareWith => "與 {name} 分享此伺服器",
        M::FriendsNotVerified => "未驗證",
        M::FriendsMinecraft => {
            "Minecraft：{names}。如果伺服器開啟了白名單，分享時會把他們加入白名單。"
        }
        M::FriendsWhitelisted => "已將 {names} 加入伺服器白名單",
        M::FriendsWhitelistFailed => "已與 {name} 分享，但無法把對方加入伺服器白名單：{error}",
        M::FriendsConnected => "已連線",
        M::FriendsSyncFailed => "無法將好友名單同步到此核心：{error}",
        M::FriendsOpenFailed => "無法開啟你的好友名單：{error}",
        M::FriendsPublishFailed => "無法通知 {name} 此核心：{error}",
        M::FriendsPublishFailedAll => "無法通知好友此核心：{error}",
        M::FriendsRetry => "重試",
        M::FriendsRetrying => "Lodestone Host 每分鐘會自動重試。",
        M::FriendsSynced => "好友名單已同步",
        M::FriendsRetired => "你的好友身分已停用",
        M::FriendsRetiredHint => {
            "此核心不再與任何人共享。要重新開始，請在 Lodestone Launcher 的好友區塊選擇新名稱，然後重新加入好友。"
        }
        M::FriendsOff => "此測試副本已關閉與好友共享",
        M::FriendsOffHint => {
            "它的核心或好友名單在測試資料夾中，而另一個不在，因此你真正的好友永遠不會進入測試核心。要測試共享，請將 LODESTONE_PATH 和 LODESTONE_LAUNCHER_DATA_DIR 都設為測試資料夾。"
        }
        M::FriendsConfirmUnverified => {
            "你還沒有和 {name} 核對安全碼，所以無法確定真的是對方：別人也可能回應了你的好友連結。請先在 Lodestone Launcher 中核對安全碼，或仍然共享。"
        }
        M::FriendsShareAnyway => "仍然共享",
        M::FriendsCertificate => "此核心的好友憑證",
        M::FriendsCertificateHint => {
            "好友透過它識別此核心。如果這台電腦的資料可能遭到複製，請停用它：好友會放棄它，核心會取得新憑證，好友改用新憑證找到它。"
        }
        M::FriendsCertificateRetire => "停用憑證…",
        M::FriendsCertificateRetireBody => {
            "好友會放棄此核心目前的憑證，並不再信任其伺服器的模組。核心會取得新憑證，好友下次檢查時會透過新憑證找到它。目前連線的所有人都會中斷連線。"
        }
        M::FriendsCertificateRetireConfirm => "停用並取得新憑證",
        M::FriendsCertificateRetired => "此核心的好友憑證已停用",
        M::FriendsCertificateRetiredHint => {
            "好友現在看不到此核心。為它取得新憑證，即可再次從這台電腦共享。"
        }
        M::FriendsCertificateNew => "取得新憑證",
        M::FriendsCertificateManual => {
            "好友現在看不到此核心。此核心無法自行產生新憑證：請結束 Lodestone Host，刪除 {path}，然後重新開啟。"
        }
        M::FriendsCancel => "取消",
        M::MemoryKeywords => "記憶體 RAM",
        M::JvmArgsKeywords => "JVM Java 參數 選項",
        M::WhitelistKeywords => "白名單 玩家",
        M::MotdKeywords => "MOTD 伺服器描述 訊息",
        M::AutoStartKeywords => "開機 啟動 自動",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModsDownloads => ["{count} 次下載", "{count} 次下載"],
        P::ModsFollowers => ["{count} 位追蹤者", "{count} 位追蹤者"],
        P::ModsCountMod => [
            "{directory}/ 中有 {count} 個模組",
            "{directory}/ 中有 {count} 個模組",
        ],
        P::ModsCountPlugin => [
            "{directory}/ 中有 {count} 個外掛",
            "{directory}/ 中有 {count} 個外掛",
        ],
        P::ModsPackTitle => [
            "玩家需要 {count} 個模組才能加入",
            "玩家需要 {count} 個模組才能加入",
        ],
        P::ModsImportHintJars => [
            "將其匯入 Prism Launcher、Modrinth App 或 ATLauncher，即可取得所有正確版本的內容。有 {count} 個模組不在 Modrinth 上，因此其檔案已包含在模組包中。",
            "將其匯入 Prism Launcher、Modrinth App 或 ATLauncher，即可取得所有正確版本的內容。有 {count} 個模組不在 Modrinth 上，因此其檔案已包含在模組包中。",
        ],
        P::ModsPackShowAll => ["顯示全部 {count} 個模組", "顯示全部 {count} 個模組"],
        P::ModsShowMore => ["顯示更多（還有 {count} 個）", "顯示更多（還有 {count} 個）"],
        P::ModsMoreGameVersions => ["還有 {count} 個", "還有 {count} 個"],
        P::ModpackAdded => ["已新增 {count} 個檔案", "已新增 {count} 個檔案"],
        P::ModpackReplaced => ["已取代 {count} 個檔案", "已取代 {count} 個檔案"],
        P::ModpackRemoved => ["已刪除 {count} 個檔案", "已刪除 {count} 個檔案"],
        P::ModpackKeptCount => [
            "保留了 {count} 個已修改的檔案",
            "保留了 {count} 個已修改的檔案",
        ],
        P::ModpackMoreChanges => ["還有 {count} 個", "還有 {count} 個"],
        P::FriendsConnectedCount => ["{count} 位好友已連線", "{count} 位好友已連線"],
    }
}
