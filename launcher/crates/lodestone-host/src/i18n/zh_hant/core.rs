use crate::i18n::{CoreMsg as M, CorePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::SectionCore => "核心",
        M::SectionPublic => "公開伺服器",
        M::SectionPublicLead => "無須登入即可查看。",
        M::SectionInfo => "資訊",
        M::SectionDanger => "危險區域",
        M::SectionDangerLead => "請謹慎變更。",
        M::CoreName => "核心名稱",
        M::CoreNameEmpty => "名稱不能是空白",
        M::CoreNameTooLong => "名稱不能超過 {max} 個字元",
        M::Domain => "公開網域或 IP",
        M::DomainTooLong => "網域不能超過 {max} 個字元",
        M::DomainHint => "如果你已設定連接埠轉送，這裡填入你的公開 IP。否則請維持不變。",
        M::ListPublicly => "公開列出伺服器",
        M::ListPubliclyHint => "任何能連到此核心的人都能看到你在下方設為公開的伺服器。",
        M::LocalClaim => "允許本機上的應用程式取得擁有者權限",
        M::LocalClaimHint => {
            "Lodestone Launcher 可以藉由證明自己能寫入此核心的資料夾來成為擁有者。能寫入該資料夾的程式本來就能修改所有使用者和設定，所以這只在共用電腦上有影響。"
        }
        M::SafeMode => "安全模式",
        M::SafeModeHint => "僅擁有者可取得此電腦的管理權限。",
        M::SafeModeOffTitle => "要關閉安全模式嗎？",
        M::SafeModeOffBody => {
            "確定要關閉安全模式嗎？這樣你就能讓自己以外的使用者執行可能有危險的指令。請確認你信任所有取得這些權限的使用者。"
        }
        M::TurnOff => "關閉",
        M::CodeExecution => "程式碼執行",
        M::CodeExecutionHint => "巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令。",
        M::CodeExecutionHintJavaOnly => "自訂 Java 指令。",
        M::CodeExecutionLocked => {
            "已被此核心的管理員鎖定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。"
        }
        M::CodeExecutionOffTitle => "要關閉程式碼執行嗎？",
        M::CodeExecutionOffBody => {
            "執行中的巨集和 Atom 伺服器將會停止。在你重新開啟程式碼執行之前，巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令都不會在此核心上執行。任何項目都不會自動重新啟動。"
        }
        M::CodeExecutionOffBodyJavaOnly => {
            "在你重新開啟程式碼執行之前，自訂 Java 指令不會在此核心上執行。任何項目都不會自動重新啟動。"
        }
        M::NoScripting => "此核心建置時未包含指令碼支援（Deno），因此無法使用巨集。",
        M::LanHttp => "允許透過未加密 HTTP 進行區域網路存取",
        M::LanHttpHint => "未加密。你網路中的其他人可以讀取密碼。",
        M::LanHttpConfirmTitle => "要允許其他裝置使用未加密 HTTP 嗎？",
        M::LanHttpConfirmBody => {
            "你網路中的裝置將能透過未加密 HTTP 連線。以這種方式傳送的密碼和工作階段權杖會以未加密的方式經過網路，網路上的其他任何人都能讀取並接管此核心上的帳戶。只有在某台裝置無法使用 https:// 時才允許此項。"
        }
        M::LanHttpAllow => "允許",
        M::LanHttpWarningOwner => {
            "其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請在下方關閉「{setting}」，並改用 https:// 連線。"
        }
        M::LanHttpWarningOther => {
            "其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請使用 https:// 連線，並請擁有者關閉未加密 HTTP 存取。"
        }
        M::OpenPort => "開放連接埠",
        M::OpenPortHint => "使用 {upnp} 在路由器上設定連接埠轉送。",
        M::OpenPortNoPermission => "沒有權限",
        M::OpenPortButton => "開放連接埠",
        M::OpenPortPort => "要開放的連接埠",
        M::PortNotNumber => "連接埠必須是數字",
        M::PortRequired => "請輸入連接埠",
        M::PortRange => "連接埠必須介於 1 到 65535 之間",
        M::PortOpened => "已開放連接埠 {port}",
        M::InfoCoreVersion => "核心版本",
        M::InfoHostVersion => "Lodestone Host 版本",
        M::InfoOs => "作業系統",
        M::InfoArch => "架構",
        M::InfoCpu => "CPU",
        M::InfoCpuCores => "CPU 核心數",
        M::InfoTotalRam => "記憶體總量",
        M::InfoNoRam => "沒有記憶體？你是怎麼執行 Lodestone 的？",
        M::InfoHttps => "HTTPS 憑證",
        M::InfoFingerprint => "憑證指紋（SHA-256）",
        M::TlsSelfSigned => "內建自我簽署憑證",
        M::TlsUserProvided => "你自己的憑證（tls/cert.pem）",
        M::TlsNone => "無法使用，僅限未加密 HTTP",
        M::ReadOnly => "只有擁有者可以變更這些設定。",
        M::ListingTitle => "分享",
        M::ListingLead => "誰可以在不登入的情況下找到此伺服器。",
        M::ListedPublicly => "公開列出",
        M::ListingPublic => "公開",
        M::ListingOwnerOnly => "只有擁有者可以變更此項",
        M::FlagStatus => "狀態和玩家",
        M::FlagAddress => "加入位址",
        M::FlagClientPack => "用戶端模組下載",
        M::ShownText => "任何能存取此核心的人無需登入即可看到此伺服器的{list}。",
        M::ShownNameVersion => "名稱和版本",
        M::ShownStatus => "狀態和玩家數",
        M::ShownAddress => "位址",
        M::ShownClientPack => "用戶端模組",
        M::TurnOnFirst => "請先開啟「公開列出伺服器」。",
        M::HiddenWhileOff => "{shown}「公開列出伺服器」關閉時會隱藏。",
        M::NotShown => "未登入時不會顯示。",
        M::NoJavaServers => "還沒有 Minecraft Java Edition 伺服器。",
        M::UsersDescription => "此核心上的帳戶及其權限。",
        M::AllUsers => "所有使用者",
        M::AllMembers => "所有成員",
        M::AllMembersCount => "所有成員（{count}）",
        M::CreateUser => "建立新使用者",
        M::CreateUserTitle => "建立新的使用者帳戶",
        M::CreateUserLead => "新使用者初始沒有任何權限。",
        M::LoadingUsers => "正在載入使用者",
        M::Permissions => "權限",
        M::PermissionsLead => "{username} 在此核心上可以執行的操作。",
        M::UnsafePermissions => "不安全的權限",
        M::UnsafePermissionsLead => "關閉安全模式後才能授予非擁有者。",
        M::OwnerOnly => "巨集、擴充功能和 Atom 伺服器",
        M::OwnerOnlyHint => "僅限擁有者。Docker 伺服器使用一般權限。",
        M::UsersOwnerOnly => "只有擁有者可以管理使用者。",
        M::You => "你",
        M::Admin => "管理員",
        M::Owner => "擁有者",
        M::ChangePasswordMenu => "變更密碼",
        M::CantDeleteSelf => "你無法刪除自己。",
        M::DeleteUserTitle => "刪除使用者",
        M::DeleteUserConfirm => "確定要刪除 {name} 嗎？此動作無法復原。",
        M::DeleteUser => "刪除使用者",
        M::NoServersYet => "還沒有伺服器。",
        M::PermCreateInstance => "建立伺服器",
        M::PermCreateInstanceUser => "此使用者可以建立新的伺服器。",
        M::PermCreateInstanceKey => "此金鑰可以建立新的伺服器。",
        M::PermDeleteInstance => "刪除伺服器",
        M::PermDeleteInstanceUser => "此使用者可以刪除任何伺服器。",
        M::PermDeleteInstanceKey => "此金鑰可以刪除任何伺服器。",
        M::PermReadGlobalFile => "讀取全域檔案",
        M::PermReadGlobalFileUser => "此使用者可以讀取執行 Lodestone Core 的電腦上的任何檔案。",
        M::PermReadGlobalFileKey => "此金鑰可以讀取執行 Lodestone Core 的電腦上的任何檔案。",
        M::PermViewInstance => "檢視伺服器",
        M::PermViewInstanceUser => "此使用者可以看到這些伺服器。",
        M::PermViewInstanceKey => "此金鑰可以看到這些伺服器。",
        M::PermStartInstance => "啟動伺服器",
        M::PermStartInstanceUser => "此使用者可以啟動這些伺服器。",
        M::PermStartInstanceKey => "此金鑰可以啟動這些伺服器。",
        M::PermStopInstance => "停止伺服器",
        M::PermStopInstanceUser => "此使用者可以停止這些伺服器。",
        M::PermStopInstanceKey => "此金鑰可以停止這些伺服器。",
        M::PermConsole => "存取伺服器主控台",
        M::PermConsoleUser => {
            "此使用者可以讀取這些伺服器的主控台並傳送指令。這實際上等於授予伺服器的管理員（OP）權限。"
        }
        M::PermConsoleKey => {
            "此金鑰可以讀取這些伺服器的主控台並傳送指令。這實際上等於授予伺服器的管理員（OP）權限。"
        }
        M::PermInstanceSettings => "存取伺服器設定",
        M::PermInstanceSettingsUser => "此使用者可以讀取和變更這些伺服器的設定。",
        M::PermInstanceSettingsKey => "此金鑰可以讀取和變更這些伺服器的設定。",
        M::PermReadResource => "讀取伺服器資源",
        M::PermReadResourceUser => "此使用者可以讀取這些伺服器的資源（世界、模組等）。",
        M::PermReadResourceKey => "此金鑰可以讀取這些伺服器的資源（世界、模組等）。",
        M::PermReadFiles => "讀取伺服器檔案",
        M::PermReadFilesUser => "此使用者可以讀取這些伺服器的檔案。",
        M::PermReadFilesKey => "此金鑰可以讀取這些伺服器的檔案。",
        M::PermWriteResource => "修改伺服器資源",
        M::PermWriteResourceUser => "此使用者可以修改這些伺服器的資源（世界、模組等）。",
        M::PermWriteResourceKey => "此金鑰可以修改這些伺服器的資源（世界、模組等）。",
        M::PermWriteFiles => "修改伺服器檔案",
        M::PermWriteFilesUser => "此使用者可以修改這些伺服器的檔案。",
        M::PermWriteFilesKey => "此金鑰可以修改這些伺服器的檔案。",
        M::PermWriteGlobalFile => "修改全域檔案",
        M::PermWriteGlobalFileUser => "此使用者可以修改執行 Lodestone Core 的電腦上的任何檔案。",
        M::PermWriteGlobalFileKey => "此金鑰可以修改執行 Lodestone Core 的電腦上的任何檔案。",
        M::PermManagePermission => "管理權限",
        M::PermManagePermissionUser => "此使用者可以管理其他使用者的權限。",
        M::PermManagePermissionKey => "此金鑰可以管理其他使用者的權限。",
        M::Username => "使用者名稱",
        M::Password => "密碼",
        M::OldPassword => "舊密碼",
        M::ConfirmPassword => "確認密碼",
        M::UsernameRequired => "請輸入使用者名稱",
        M::PasswordRequired => "請輸入密碼",
        M::ConfirmRequired => "請確認密碼",
        M::PasswordsMustMatch => "兩次輸入的密碼不一致",
        M::ChangePasswordButton => "變更密碼",
        M::CreateUserButton => "建立使用者",
        M::Account => "帳戶",
        M::ProfileUsername => "使用者名稱",
        M::ProfilePassword => "密碼",
        M::ChangePassword => "變更密碼",
        M::Appearance => "外觀",
        M::AppearanceLead => "儲存在這台電腦上，而不是核心上。",
        M::ColoredConsole => "彩色主控台",
        M::Language => "語言",
        M::LanguageAuto => "系統預設（{language}）",
        M::Theme => "主題",
        M::ThemeSystem => "跟隨系統",
        M::ThemeLight => "淺色",
        M::ThemeDark => "深色",
        M::ThemeSystemHint => "跟隨電腦的淺色或深色設定",
        M::ThemeLightHint => "一律使用淺色",
        M::ThemeDarkHint => "一律使用深色",
        M::Zoom => "縮放",
        M::ZoomHint => "也可以用 {modifier} + 和 {modifier} −。",
        M::ApiKeys => "API 金鑰",
        M::ApiKeysOwnLead => "讓 CLI、指令碼和代理程式無須密碼即可以你的身分操作。",
        M::ApiKeysOwnEmpty => "你還沒有 API 金鑰。",
        M::ApiKeysUserLead => "{username} 建立的金鑰。撤銷後，使用它的程式會立即失去存取權。",
        M::ApiKeysUserEmpty => "{username} 沒有 API 金鑰。",
        M::CreateApiKey => "建立 API 金鑰",
        M::ApiKeyCreated => "已建立 API 金鑰「{name}」",
        M::LoadingApiKeys => "正在載入 API 金鑰",
        M::Revoke => "撤銷",
        M::RevokeTitle => "撤銷 API 金鑰",
        M::RevokeBody => {
            "要撤銷「{name}」（{display}）嗎？使用此金鑰的所有程式將立即失去存取權。此操作無法復原。"
        }
        M::ApiKeyRevoked => "已撤銷 API 金鑰「{name}」",
        M::CreatedOn => "建立於 {date}",
        M::ExpiresOn => "{date} 到期",
        M::ExpiredOn => "已於 {date} 到期",
        M::ExpiresNever => "到期時間：永不",
        M::LastUsed => "上次使用：{time}",
        M::LastUsedNever => "上次使用：從未",
        M::ScopeFull => "完整存取",
        M::ScopeLauncher => "Launcher：檢視、啟動和停止伺服器",
        M::ScopeLauncherConsole => "Launcher：檢視、啟動和停止伺服器，讀取和使用主控台",
        M::ScopeLauncherConsoleRead => "Launcher：檢視、啟動和停止伺服器，讀取主控台",
        M::ScopeLauncherConsoleWrite => "Launcher：檢視、啟動和停止伺服器，傳送主控台指令",
        M::ScopeCustom => "自訂：{servers}，{permissions}",
        M::PasswordSessionOnly => {
            "只有使用密碼登入時才能管理 API 金鑰，使用 API 金鑰登入時無法管理。"
        }
        M::CreateApiKeyLead => "為每個金鑰設定較短的有效期限，並只授予需要的存取權。",
        M::ApiKeyName => "名稱",
        M::ApiKeyNameRequired => "請輸入名稱",
        M::ApiKeyNameTooLong => "名稱最多 {max} 個字元",
        M::ExpiresIn => "有效期限",
        M::Expiry30d => "30 天",
        M::Expiry90d => "90 天",
        M::Expiry1y => "1 年",
        M::ExpiryNever => "永不到期",
        M::NeverWarning => {
            "永不到期的金鑰在你撤銷之前會一直有效。除非確實需要，否則請設定有效期限。"
        }
        M::Access => "存取權",
        M::AccessFull => "完整存取（你能做的一切）",
        M::AccessCustom => "自訂",
        M::ChooseOne => "請至少選擇一項權限。",
        M::CreateKey => "建立金鑰",
        M::OnlyYours => "金鑰只能擁有你自己擁有的權限。你沒有的權限和伺服器會顯示為灰色。",
        M::NotYours => "你沒有此權限。",
        M::NoServers => "沒有你可以看到的伺服器。",
        M::KeyUnsafe => "不安全的設定",
        M::KeyUnsafeSafeMode => {
            "安全模式已開啟，因此在核心設定中關閉安全模式之前，核心會拒絕將這些權限授予自訂存取的金鑰。"
        }
        M::KeyOwnerOnly => "巨集、擴充功能和 Atom 伺服器",
        M::KeyOwnerOnlyHint => "只有完整存取的金鑰才能使用巨集、擴充功能和 Atom 伺服器。",
        M::NewKeyTitle => "已建立 API 金鑰「{name}」",
        M::NewKeyCopyNow => "請立即複製此金鑰。之後將無法再次檢視。",
        M::NewKeyUseWithCli => "在 Lodestone CLI 中使用：",
        M::NewKeyStore => {
            "請像保管密碼一樣保管此金鑰。在它到期或被你撤銷之前，任何擁有它的人都能以它的權限使用 Lodestone。"
        }
        M::KeyCopied => "已將 API 金鑰複製到剪貼簿",
        M::CliCopied => "已將 CLI 範例複製到剪貼簿",
        M::MinecraftAccounts => "Minecraft 帳戶",
        M::MinecraftOwnLead => "從 Lodestone Launcher 連結。",
        M::MinecraftOwnEmpty => {
            "尚未連結 Minecraft 帳戶。在 Lodestone Launcher 中登入此核心即可連結。"
        }
        M::MinecraftUserLead => "{username} 從 Lodestone Launcher 連結的 Minecraft 帳戶。",
        M::MinecraftUserEmpty => "{username} 沒有連結的 Minecraft 帳戶。",
        M::MinecraftTooOld => "此核心版本太舊，無法連結 Minecraft 帳戶。",
        M::MinecraftRemoveTitle => "移除 Minecraft 帳戶",
        M::MinecraftRemoveBody => {
            "要移除與 {name} 的連結嗎？之後可以從 Lodestone Launcher 重新連結。"
        }
        M::MinecraftRemoved => "已移除 {name}",
        M::Notifications => "通知",
        M::InProgress => "進行中",
        M::AllNotifications => "所有通知",
        M::LoadingNotifications => "正在載入通知",
        M::NoNotifications => "目前沒有通知！",
        M::EventStarting => "伺服器 {name} 正在啟動！",
        M::EventRunning => "伺服器 {name} 已啟動！",
        M::EventStopping => "伺服器 {name} 正在停止！",
        M::EventStopped => "伺服器 {name} 已停止！",
        M::EventCrashed => "伺服器 {name} 發生錯誤！",
        M::WarningToast => "警告：某個伺服器發出了警告。這不應該發生，請向開發者回報此問題。",
        M::EventWarning => "伺服器 {name} 發出了警告",
        M::EventError => "伺服器 {name} 發生錯誤",
        M::PlayersJoined => "{players} 加入了 {name}",
        M::PlayersLeft => "{players} 離開了 {name}",
        M::PlayersJoinedAndLeft => "{joined} 加入了 {name}，{left} 離開了 {name}",
        M::PlayerMessage => "{player} 在 {name} 上說：{message}",
        M::EventApiKeyCreated => "已建立 API 金鑰「{name}」",
        M::EventApiKeyRevoked => "已撤銷 API 金鑰「{name}」",
        M::MacroStarted => "巨集 {pid} 已在 {server} 上啟動",
        M::MacroDetached => "巨集 {pid} 已在 {server} 上分離",
        M::MacroStopped => "巨集 {pid} 已在 {server} 上停止，狀態為 {status}",
        M::LoadFailed => "無法載入通知：{error}",
        M::CoreFallback => "核心",
        M::CoreFailedNotice => "Lodestone Core 無法啟動：{error}",
        M::UpdatesTitle => "更新",
        M::UpdatesLead => "這台電腦上的 Lodestone Host。Lodestone Core 隨之更新。",
        M::ThisVersion => "此版本",
        M::UpdateAutomatically => "自動更新",
        M::UpdateAutomaticallyHint => "在背景下載新組建。",
        M::BetaBuilds => "測試版組建",
        M::BetaBuildsHint => {
            "搶先體驗新功能。測試版組建可能有錯誤。關閉後會保留目前的組建，直到有更新的穩定版組建。"
        }
        M::DeveloperBuilds => "開發者組建",
        M::DeveloperBuildsHint => {
            "供 Lodestone 開發者使用：更新到開發中的組建，可能無法正常運作。關閉後會保留目前的組建，直到有更新的正式發行。"
        }
        M::BuildNumber => "組建 {build}",
        M::BuildNumberCommit => "組建 {build} · {commit}",
        M::DevelopmentBuild => "開發組建",
        M::UpdatesUnavailable => "此組建不會自動更新。從官網下載的 Lodestone Host 會自動更新。",
        M::UpdateNotChecked => "每隔幾小時檢查一次是否有新組建。",
        M::UpdateChecking => "正在檢查新組建…",
        M::UpToDate => "目前已是最新組建。",
        M::UpdateAvailable => "組建 {build} 現已可用。",
        M::UpdateDownloading => "正在下載組建 {build}… {percent}",
        M::UpdateReady => {
            "組建 {build} 已下載，重新啟動後即可使用：你的伺服器會停止，設為自動啟動的伺服器隨後會再次啟動。結束時也會安裝。"
        }
        M::RestartUpdateTitle => "重新啟動以更新？",
        M::RestartUpdateBody => {
            "重新啟動 Lodestone Host 會先停止這些伺服器。設為自動啟動的伺服器會在重新啟動後再次啟動。"
        }
        M::RestartUpdateConfirm => "停止伺服器並重新啟動",
        M::UpdateFailed => "無法更新：{error}",
        M::CheckForUpdates => "立即檢查",
        M::DownloadUpdate => "下載",
        M::RestartToUpdate => "重新啟動",
        M::BetaTitle => "感謝使用 Lodestone 測試版！",
        M::BetaBody => {
            "感謝使用 Lodestone 測試版！如果你發現任何問題，請在我們的 Discord 伺服器或 Github 上告訴我們。"
        }
        M::Cores => "核心",
        M::CoresLead => {
            "Lodestone Host 執行這台電腦上的核心。你也可以管理另一個核心，例如伺服器上的核心。"
        }
        M::ThisComputerCore => "這台電腦",
        M::ThisComputerHint => "Lodestone Host 執行的核心。",
        M::CurrentCore => "目前",
        M::UseCore => "切換",
        M::ChangeCore => "更換核心",
        M::ConnectToCore => "連線到核心",
        M::CoreAddress => "位址",
        M::CoreAddressHint => "例如 192.168.1.5 或 core.example.com:16662。",
        M::Connect => "連線",
        M::TrustTitle => "信任此核心的憑證嗎？",
        M::TrustBody => {
            "{host} 使用自己的 HTTPS 憑證，這台電腦不認得它。Lodestone 核心會自己產生憑證，所以這是正常的：請核對它的指紋，然後信任它。"
        }
        M::FingerprintCaption => "憑證指紋（SHA-256）",
        M::FingerprintHelp => {
            "請將它與該核心「設定 › 一般 › 資訊」中的指紋，或它啟動時日誌中的指紋比對。"
        }
        M::TrustCertificate => "信任憑證",
        M::SignInTo => "登入到 {name}",
        M::VersionIncompatible => {
            "此 Core 執行 Lodestone Core {theirs}，本機執行 {ours}。在兩者都更新之前，某些頁面可能無法使用。"
        }
        M::VersionPatch => "此 Core 執行 Lodestone Core {theirs}（本機：{ours}）。",
        M::SignIn => "登入",
        M::RemoveCore => "移除",
        M::RemoteSignedOut => "你已從 {name} 登出。Lodestone Host 已回到這台電腦的核心。",
        M::RemoteAccess => "允許其他裝置存取",
        M::RemoteAccessHintOff => {
            "關閉：只有這台電腦上的應用程式可以使用這個核心（Lodestone Host、Lodestone Launcher 和 Lodestone CLI）。開啟後可以在其他裝置上使用網頁控制台、新增使用者或建立 API 金鑰。無論開關如何，都可以和朋友分享伺服器。"
        }
        M::RemoteAccessHintOn => "其他裝置可以透過 HTTPS 用使用者名稱和密碼登入這個核心。",
        M::RemoteAccessOffTitle => "關閉其他裝置的存取？",
        M::RemoteAccessOffBody => {
            "只有這台電腦上的應用程式可以使用這個核心。其他裝置、其他使用者和在別處使用的 API 金鑰將無法使用，直到你重新開啟。和朋友分享伺服器仍然可用。"
        }
        M::LocalClaimNeedsPassword => "請先設定擁有者帳戶：沒有密碼時，這是唯一的登入方式。",
        M::OwnerAccountTitle => "設定擁有者帳戶以分享這個核心",
        M::OwnerAccountLead => {
            "其他裝置使用使用者名稱和密碼登入。請選擇你將在網頁控制台和其他裝置上使用的使用者名稱和密碼。這台電腦上的應用程式無需它們也能繼續登入。"
        }
        M::OwnerAccountButton => "設定並允許存取",
        M::OwnerAccountDone => "擁有者帳戶已設定。其他裝置現在可以登入這個核心。",
        M::PasswordTooShort => "密碼至少需要 12 個字元。",
        M::SetPassword => "設定密碼",
        M::NoPasswordYet => "還沒有密碼。只有這台電腦上的應用程式可以使用這個核心。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScopeServers => ["{count} 個伺服器", "{count} 個伺服器"],
        P::ScopePermissions => ["{count} 項權限", "{count} 項權限"],
        P::ServersRunning => ["{count} 個伺服器正在執行。", "{count} 個伺服器正在執行。"],
    }
}
