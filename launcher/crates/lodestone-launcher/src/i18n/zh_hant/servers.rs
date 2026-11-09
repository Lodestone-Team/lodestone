use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::PageHint => "你的 Lodestone 核心上的伺服器。建立與其中一個相符的實例，然後在上面遊玩。",
        M::OnThisComputer => "這台電腦上",
        M::OnThisComputerHint => {
            "在這裡執行的 Lodestone Host 和 Lodestone Core。以擁有者身分連線時，啟動器會證明它能存取核心的檔案，因此不需要密碼。"
        }
        M::Rescan => "重新掃描",
        M::ScanningLocal => "正在尋找這台電腦上的核心…",
        M::NoLocalCores => "這台電腦上沒有正在執行的 Lodestone Host 或 Lodestone Core。",
        M::ConnectAsOwner => "以擁有者身分連線",
        M::Connected => "已連線",
        M::ChooseDataFolder => "選擇資料夾…",
        M::ChooseDataFolderPrompt => "選擇",
        M::ProductHostHint => "桌面應用程式，控制面板在視窗中",
        M::ProductCoreHint => "獨立的核心（伺服器程式或 Docker）",
        M::ConnectedAsOwner => "已以擁有者身分連線到 {name}",
        M::ConnectedMadeOwner => {
            "它還沒有擁有者，因此 Lodestone Launcher 建立了擁有者帳號 {username}。密碼顯示在核心的卡片上，請妥善保存。"
        }
        M::ConnectedOwnerKey => "此啟動器現在擁有它的擁有者金鑰。你可以在核心的控制面板中撤銷它。",
        M::OwnerSignIn => "擁有者登入",
        M::OwnerKey => "擁有者金鑰",
        M::ShowPassword => "顯示",
        M::HidePassword => "隱藏",
        M::CopyUsername => "複製使用者名稱",
        M::CopyPassword => "複製密碼",
        M::CopyApiKey => "複製 API 金鑰",
        M::StartedBy => "由 {who} 啟動",
        M::StartedDirectly => "直接啟動",
        M::DirectNote => {
            "此核心是直接啟動的。Lodestone Core 應透過 lodestone_cli 執行，它會安裝核心並保持更新。"
        }
        M::GetLodestoneCli => "取得 lodestone_cli",
        M::CredentialsReadFailed => "無法讀取這台電腦上核心的擁有者金鑰",
        M::CredentialsSaveFailed => "無法儲存擁有者金鑰",
        M::ClaimNoCore => "這台電腦上沒有正在執行的 Lodestone Host 或 Lodestone Core。",
        M::ClaimRefused => {
            "核心拒絕授予擁有者權限。請在它的設定中開啟「允許這台電腦上的應用程式取得擁有者權限」，或更新它。"
        }
        M::ClaimFolderNotFound => {
            "找不到此核心的資料夾。請選擇它存放資料的資料夾（它的 LODESTONE_PATH）。在 Docker 中執行的核心需要從這台電腦掛載資料夾。"
        }
        M::ClaimNotThisCore => "回應的核心並不是找到的資料夾所屬的核心，因此沒有儲存任何內容。",
        M::ClaimNoResult => "核心沒有完成認領。請再試一次。",
        M::ClaimNoCertificate => {
            "此核心沒有 HTTPS 憑證，因此 Lodestone Launcher 無法確保它的擁有者金鑰只會傳送給它。請在核心的設定中開啟 HTTPS，然後重新連線。"
        }
        M::ClaimFiles => "無法使用核心的資料夾：{error}",
        M::ClaimInvalid => "核心傳送了 Lodestone Launcher 無法理解的內容：{error}",
        M::AddServer => "新增伺服器",
        M::Dismiss => "關閉",
        M::NoServersYet => "還沒有伺服器",
        M::NoServersHint => {
            "依位址新增一個 Lodestone 核心。不登入時，你會看到其擁有者公開列出的伺服器。"
        }

        M::BadgeTrusted => "已信任",
        M::BadgePinned => "憑證已釘選",
        M::SignedInAs => "已用 {username} 登入",
        M::SignedIn => "已登入",
        M::NotSignedIn => "未登入：僅顯示公開伺服器",
        M::TrustedForMods => "信任此核心的模組",
        M::Offline => "離線：無法連線到此核心。請確認它正在執行，且位於同一個網路。",
        M::NotListedPublicly => {
            "擁有者尚未在核心設定中開啟「公開列出伺服器」，因此不登入就沒有可顯示的內容。"
        }
        M::NoPublicServers => "沒有公開列出的伺服器。登入後即可查看你有權存取的伺服器。",
        M::NoVisibleServers => "此核心上沒有你看得到的伺服器。",
        M::CertChangedWarning => {
            "此核心的 HTTPS 憑證在你信任之後已變更，因此 Lodestone Launcher 拒絕連線。核心會在內建憑證即將到期或位址變更時產生新憑證；但這也可能表示有人正在攔截連線。信任新憑證前，請先核對它的指紋。"
        }
        M::CertUntrustedWarning => {
            "{computer} 不信任此核心的 HTTPS 憑證。Lodestone 核心會自行產生憑證，因此這是正常的：請核對它的指紋，然後信任它。"
        }
        M::ReviewNewCertificate => "信任新憑證…",
        M::ReviewCertificate => "信任憑證…",

        M::Details => "詳細資料",
        M::CreateInstance => "建立實例",
        M::StartAndJoin => "啟動並加入",
        M::Play => "開始遊戲",
        M::More => "更多…",
        M::InstanceReady => "實例已就緒",
        M::AddressHidden => "擁有者未分享此伺服器的位址。",

        M::StateStarting => "啟動中",
        M::StateRunning => "執行中",
        M::StateStopping => "停止中",
        M::StateStopped => "已停止",
        M::StateCrashed => "已當機",
        M::StateUnknown => "不明",
        M::StateHidden => "狀態未公開",
        M::LoaderOther => "其他",
        M::PlayersOfMax => "{count}/{max} 位玩家",

        M::BusyFetching => "正在取得伺服器的模組…",
        M::BusyCreating => "正在建立實例並下載…",
        M::BusyCreatingFiles => "正在建立實例，已下載 {done}/{total} 個檔案…",
        M::BusyChecking => "正在檢查伺服器的模組…",
        M::BusyUpdating => "正在更新模組…",
        M::BusyUpdatingFiles => "正在更新模組，{done}/{total} 個檔案…",
        M::BusyStarting => "正在啟動伺服器…",
        M::BusyStopping => "正在停止…",

        M::ReadFailed => "無法讀取你的伺服器",
        M::SaveFailed => "無法儲存你的伺服器",
        M::NoLongerTrusted => "已不再信任 {name} 的模組",
        M::NoLongerTrustedIdChanged => {
            "現在由另一個核心在 {address} 回應（它的 ID 已變更）。只有在你信任其擁有者時，才在它的卡片上重新信任它。"
        }
        M::NoLongerTrustedOtherCore => "現在由另一個核心在 {address} 回應。",
        M::NoLongerTrustedNewCertificate => {
            "你為它信任了新的憑證。如果確定它是同一個核心，請在它的卡片上重新信任其模組。"
        }
        M::NoLongerTrustedCertificateChanged => {
            "它的憑證已變更。如果確定它是同一個核心，請在它的卡片上重新信任其模組。"
        }
        M::SignedOutOf => "已登出 {name}",
        M::KeyStillValid => {
            "此裝置的金鑰（lsk_{id}_…）在到期前仍然有效。若要立即讓它失效，請在控制面板的「設定 → API 金鑰」中撤銷它。"
        }
        M::KeySaveFailed => "無法將金鑰儲存到{keychain}",
        M::KeySaveFailedError => "無法將金鑰儲存到{keychain}：{error}",
        M::CheckCertificate => "請核對核心的憑證以繼續。",
        M::CertificateNotTrusted => "憑證未獲信任，因此你的登入資訊沒有傳送給核心。",
        M::CreateFailed => "無法建立實例",
        M::CreatedWithoutMods => "已建立 {name}，但不含伺服器的模組",
        M::CreatedWithoutModsBody => {
            "擁有者未分享此伺服器的用戶端模組，因此這個實例只有它的 Minecraft 版本和載入器。請自行加入它需要的模組。"
        }
        M::LinkFailed => "無法將實例連結到伺服器",
        M::ReadyTitle => "{name} 已就緒",
        M::ReadyBody => {
            "Minecraft {version}，使用 {loader}，並包含伺服器的模組和設定檔。按下「開始遊戲」即可加入。"
        }
        M::CheckModsFailed => "無法檢查伺服器的模組",
        M::JoinFailed => "無法加入 {name}",
        M::CoreRemoved => {
            "建立此實例時所用的核心（{address}）已不在 Lodestone Launcher 中。請在「Lodestone Host」頁面重新新增它，或直接按「開始遊戲」而不加入伺服器。"
        }
        M::ServerNotShown => "核心不再向你顯示此伺服器：它已被刪除，或者它沒有公開列出而你未登入。",
        M::NotRunning => "{name} 未在執行",
        M::AskOwnerToStart => "請它的擁有者啟動它，或在「Lodestone Host」頁面登入核心後自行啟動。",
        M::UpdateModsFailed => "無法更新模組",
        M::StopGameFirst => "請先結束遊戲。",
        M::UpdateInstanceFailed => "無法更新實例",
        M::InstanceNotSaved => "模組已更新，但無法儲存實例：{error}",
        M::BadJoinAddress => "Minecraft 無法加入此伺服器的位址或連接埠。",
        M::StartCrashed => "伺服器在啟動時當機了。請在控制面板中查看它的主控台。",
        M::StartStopped => "伺服器在啟動時停止了。請在控制面板中查看它的主控台。",
        M::ServerGone => "此伺服器已不存在。",
        M::StartTimedOut => "伺服器未能在 5 分鐘內完成啟動。",
        M::StartFailed => "無法啟動 {name}",
        M::StopFailed => "無法停止 {name}",
        M::UnlinkFailed => "無法取消連結實例：{error}",
        M::Cancelled => "已取消",
        M::TrustExplanation => {
            "來自 Lodestone 核心的模組會以程式的形式在{computer} 上執行，擁有與你相同的檔案和帳號存取權限。只信任你信得過其擁有者的核心。"
        }
        M::FingerprintHelp => {
            "請將它與核心顯示的指紋比對：在控制面板的「設定 → 核心設定 → 資訊」中（「憑證指紋（SHA-256）」），或在核心啟動時的日誌中（「TLS certificate SHA-256 fingerprint」）。"
        }

        M::CoreUnreachable => "無法連線到核心。請檢查位址，並確認核心正在執行。（{detail}）",
        M::CoreUntrusted => {
            "{computer} 不信任 {host} 的 HTTPS 憑證。請核對它的指紋，然後信任它以進行連線。"
        }
        M::CoreCertificateChanged => {
            "{host} 的 HTTPS 憑證在你信任之後已變更，因此 Lodestone Launcher 拒絕連線。信任新憑證前，請先核對它的指紋。"
        }
        M::CoreSignedOut => "已登出：此裝置的金鑰已被撤銷或已到期。請重新登入。",
        M::CoreBadCredentials => "使用者名稱或密碼錯誤。",
        M::CoreForbidden => "核心拒絕了要求：你沒有執行此操作的權限。",
        M::CoreNotFound => "找不到",
        M::CoreTooOld => {
            "此核心的版本太舊，無法搭配 Lodestone Launcher 使用。請將 Lodestone Core 更新到支援伺服器清單的版本。"
        }
        M::CoreTooManyRequests => "嘗試次數過多。請幾分鐘後再試。",
        M::CoreAnswered => "核心回應了 {status}：{message}",
        M::CoreInvalidReply => "核心傳送了 Lodestone Launcher 無法解讀的回應：{detail}",

        M::DifferentMods => "{name} 的模組有所不同",
        M::DifferentModsBody => {
            "自此實例取得伺服器的模組以來，它們已有變動（{changes}）。請先更新才能加入；你自己的模組會維持原樣，被取代的檔案會保留在實例的備份中。"
        }
        M::ChangeNew => "新增：{name}",
        M::ChangeChanged => "已變更：{name}",
        M::ChangeRemoved => "已移除：{name}",
        M::ChangeConfig => "設定檔：{name}",
        M::NoChanges => "沒有變更",
        M::UpdateAndPlay => "更新並開始遊戲",
        M::PlayAnyway => "仍要開始遊戲",

        M::CertChangedTitle => "{name} 的憑證已變更",
        M::CertTrustTitle => "要信任此核心的憑證嗎？",
        M::CertChangedIntro => {
            "位於 {address} 的核心出示的 HTTPS 憑證與你信任的不同。核心會在內建憑證即將到期或位址變更時產生新憑證，但這也可能表示有人正在攔截連線。"
        }
        M::CertUntrustedIntro => {
            "{os} 不信任 {name} 在 {address} 上使用的 HTTPS 憑證。Lodestone 核心會自行產生憑證，因此第一次連線時出現這種情況是正常的。"
        }
        M::FingerprintTrustedBefore => "先前信任的憑證（SHA-256）",
        M::FingerprintPresented => "目前出示的憑證（SHA-256）",
        M::FingerprintCaption => "憑證指紋（SHA-256）",
        M::CertIfMatch => {
            "如果兩者相符，就信任它：Lodestone Launcher 之後只會為 {host} 接受這一個憑證，並在它變更時提醒你。"
        }
        M::CertIfMatchChanged => {
            "如果兩者相符，就信任它：Lodestone Launcher 之後只會為 {host} 接受這一個憑證，並在它變更時提醒你。已信任其模組的核心會改為不受信任，直到你重新信任它。"
        }
        M::TrustNewCertificate => "信任新憑證",
        M::TrustCertificate => "信任憑證",

        M::TrustTitle => "你信任 {name} 嗎？",
        M::CoreAt => "{name}，位址為 {address}。",
        M::TrustAfterAdd => {
            "在你信任它之前，Lodestone Launcher 會在安裝伺服器的模組前先讓你看過它們。你之後可以用核心卡片上的「{setting}」變更此設定。"
        }
        M::TrustLater => {
            "之後來自此核心的模組會直接安裝，不再詢問。你可以在核心卡片上再次關閉此設定。"
        }
        M::TrustThisCore => "信任此核心",

        M::AdvisoryTitle => "此伺服器的模組將在{computer} 上執行",
        M::AdvisoryNew => {
            "{core} 上 {server} 的實例會取得這些檔案。模組就是程式：它們執行時擁有與你相同的檔案和帳號存取權限。"
        }
        M::AdvisoryUpdate => {
            "更新為 {core} 上 {server} 的模組會安裝這些檔案。模組就是程式：它們執行時擁有與你相同的檔案和帳號存取權限。"
        }
        M::ModrinthUnreachable => "無法連線到 Modrinth，因此無法核對任何檔案。",
        M::UploadedNotChecked => "上傳到伺服器的檔案 — 未經 Modrinth 核對（{count}）",
        M::AlsoOnModrinth => "也在 Modrinth 上（{count}）",
        M::AlsoFiles => "另有 {files}。",
        M::AlsoFilesScripts => "另有 {files}。腳本檔案（KubeJS、CraftTweaker）同樣可以驅動程式碼。",
        M::OnlyIfYouTrust => "只有在你信任 {core} 的擁有者時才繼續。",
        M::AdvisoryTrustNote => {
            "「{button}」也會讓之後來自它的模組直接安裝，不再詢問。{explanation}"
        }
        M::InstallThisTime => "僅這次安裝",
        M::TrustAndInstall => "信任 {core} 並安裝",

        M::KeyDays90 => "90 天",
        M::KeyYear => "1 年",
        M::KeyNever => "永不到期",
        M::AddressPlaceholder => "192.168.1.20 或 localhost:16662",
        M::UsernameOptional => "使用者名稱（選填）",
        M::Username => "使用者名稱",
        M::Password => "密碼",
        M::EnterAddress => "請輸入核心的位址。",
        M::EnterUsernamePassword => "請輸入你的使用者名稱和密碼。",
        M::EnterPassword => "請輸入密碼，或將使用者名稱留空。",
        M::ConsoleAccess => "主控台存取（選填）",
        M::ReadConsole => "讀取主控台",
        M::SendChat => "傳送聊天和指令",
        M::ConsoleAccessHint => {
            "僅在你的使用者有權使用主控台的伺服器上有效。指令能做管理員能做的任何事，因此只有在你信任所有使用{computer} 的人時才允許。"
        }
        M::SignInTitle => "登入伺服器",
        M::SignInIntro => {
            "登入後即可啟動和停止伺服器，並查看沒有公開列出的伺服器。Lodestone Launcher 會為{computer} 產生專用金鑰；你的密碼不會被儲存。"
        }
        M::AddIntro => "Lodestone 核心的位址。不登入時，你會看到其擁有者公開列出的伺服器。",
        M::Address => "位址",
        M::SignInOptional => "登入（選填）",
        M::KeyExpiresAfter => "{computer} 上的金鑰有效期限",
        M::NeverExpiresWarning => "永不到期的金鑰會一直有效，直到你在控制面板中撤銷它。",

        M::ChatPlaceholder => "說點什麼，或輸入 /指令 來執行",
        M::ConsoleNotAllowedAtSignIn => "{computer} 登入時未允許讀取主控台。",
        M::ConsoleKeyCantRead => {
            "{computer} 上的金鑰無法讀取主控台。如果你的使用者可以使用主控台，請重新登入並允許「{option}」。"
        }
        M::ConsoleKeyCantReadShort => "{computer} 上的金鑰無法讀取主控台。",
        M::ConsoleKeyCantWrite => {
            "{computer} 上的金鑰無法傳送聊天或指令。如果你的使用者可以使用主控台，請重新登入並允許「{option}」。"
        }
        M::OfflineCantLink => "離線帳號無法連結：它們的 UUID 是由名稱產生的，因此無法證明身分。",
        M::LinkedTo => "已將 {account} 連結到 {user}。",
        M::YourLodestoneUser => "你的 Lodestone 使用者",
        M::Connecting => "正在連線…",
        M::Live => "即時",
        M::Reconnecting => "正在重新連線：{reason}",
        M::ConsoleNotShown => "未顯示主控台",
        M::OutputAppearsHere => "伺服器的輸出會顯示在這裡。",
        M::Send => "傳送",
        M::ChatHint => "文字會透過 /say 傳給所有人。以 / 開頭即可執行指令。",
        M::Console => "主控台",
        M::JumpToLatest => "跳到最新",
        M::Players => "玩家",
        M::NoOneOnline => "沒有人在線上。",
        M::ServerNotRunning => "伺服器未在執行。",
        M::MinecraftAccount => "Minecraft 帳號",
        M::CoreTooOldToLink => "此核心的版本太舊，無法連結 Minecraft 帳號。",
        M::Unlink => "取消連結",
        M::LinkAccount => "將 {account} 連結到 {user}",
        M::LinkCheckHint => "核心會使用這次登入向 Microsoft 確認此帳號屬於你，只用一次，不會保留。",
        M::SignInMicrosoftToLink => {
            "使用 Microsoft 帳號登入，即可將它連結到你的 Lodestone 使用者。"
        }

        M::HostTitle => "在 Lodestone 核心上託管",
        M::ShowOnServersPage => "在「Lodestone Host」頁面查看",
        M::HostIntro => {
            "以 Modrinth 模組包的形式，用 {name} 的模組和設定檔建立伺服器。來自 Modrinth 的僅用戶端模組會被排除。伺服器建立後處於停止狀態。"
        }
        M::CoreLabel => "核心",
        M::PasswordNotKept => "你的密碼只會在登入時傳送一次，不會被儲存。",
        M::ServerName => "伺服器名稱",
        M::Memory => "記憶體",
        M::GamePortHint => "遊戲連接埠是核心所在電腦上從 {port} 起第一個空閒的連接埠。",
        M::EulaAgree => "我同意 Minecraft EULA（{url}）。Lodestone Core 會為新伺服器接受它。",
        M::CreateServer => "建立伺服器",
        M::NameTheServer => "請為伺服器命名。",
        M::HostNeedsPassword => {
            "請輸入你在此核心上的使用者名稱和密碼。建立伺服器需要比{computer} 上的金鑰更高的權限。"
        }
        M::AgreeEulaFirst => "請先同意 Minecraft EULA：核心會為新伺服器接受它。",
        M::StageExporting => "正在匯出實例…",
        M::StageSigningIn => "正在登入…",
        M::StageUploading => "正在上傳模組包（{size}）…",
        M::StageCreating => "正在建立伺服器…",
        M::CoreGone => "該核心已不存在。",
        M::InstanceGone => "該實例已不存在。",
        M::PackNotKept => "核心沒有保留該模組包。",
        M::SignedOutRightAway => "核心在你登入後立刻將你登出。請再試一次。",
        M::AddCoreFirst => "請先在「Lodestone Host」頁面新增你的 Lodestone 核心。",

        M::LanListenFailed => "無法監聽區域網路世界：{error}",
        M::LanLooking => "正在尋找在區域網路上公開的世界…",
        M::LanNoneMac => {
            "找不到區域網路世界。在 Minecraft 中開啟一個世界，然後選擇「在區域網路上公開」。如果 macOS 曾詢問是否允許 Lodestone Launcher 尋找區域網路上的裝置，請在「系統設定 → 隱私權與安全性 → 區域網路」中允許，然後重新開啟此頁面。"
        }
        M::LanNoneWindows => {
            "找不到區域網路世界。在 Minecraft 中開啟一個世界，然後選擇「在區域網路上公開」。如果 Windows 曾詢問是否允許 Lodestone Launcher 通過防火牆，請在私人網路上允許它，然後重新開啟此頁面。"
        }
        M::LanNoneOther => {
            "找不到區域網路世界。在 Minecraft 中開啟一個世界，然後選擇「在區域網路上公開」，再重新開啟此頁面。"
        }
        M::LanWorld => "區域網路世界",
        M::Join => "加入",
        M::OnYourNetwork => "你的網路",
        M::OnYourNetworkHint => "此網路上已在區域網路上公開的世界。",
        M::JoinWorld => "加入 {world}",
        M::PickInstance => {
            "選擇用來遊玩的實例。它需要與位於 {address} 的世界有相同的 Minecraft 版本和模組。"
        }
        M::NoInstancesYet => "你還沒有任何實例。請先建立一個。",

        M::ANewInstance => "新的實例",
        M::NewMatchingInstance => "新的相符實例",
        M::NoResourcePack => "此伺服器沒有設定資源包。",
        M::DownloadToInstance => "下載到實例",
        M::PackNote => "Minecraft 會在你加入時下載它；將它儲存到 {target} 中，離線時也能使用。",
        M::PackNoteRequired => {
            "玩家必須接受它才能加入。Minecraft 會在你加入時下載它；將它儲存到 {target} 中，離線時也能使用。"
        }
        M::PackNoteNoSha => {
            "伺服器沒有提供它的 SHA-1。Minecraft 會在你加入時下載它；將它儲存到 {target} 中，離線時也能使用。"
        }
        M::PackNoteRequiredNoSha => {
            "玩家必須接受它才能加入。伺服器沒有提供它的 SHA-1。Minecraft 會在你加入時下載它；將它儲存到 {target} 中，離線時也能使用。"
        }
        M::InstanceLabel => "實例",
        M::ResourcePack => "資源包",
        M::World => "世界",
        M::WorldCopyHint => {
            "伺服器世界的單人遊戲副本，放在新的世界資料夾中。你在其中所做的變更只會保留在{computer} 上。"
        }
        M::StopToDownload => "請先停止伺服器再下載它的世界：執行中的伺服器可能正在寫入它。",
        M::PasswordNotKeptDownload => "你的密碼只會在為這次下載登入時傳送一次，不會被儲存。",
        M::DownloadWorld => "下載世界",
        M::WorldNeedsPassword => {
            "請輸入你在此核心上的使用者名稱和密碼。下載世界需要讀取伺服器檔案的權限，而{computer} 上的金鑰沒有這項權限。"
        }
        M::SignInToSeePack => "登入此核心即可查看伺服器的資源包。",
        M::CreatingMatching => "正在建立相符的實例",
        M::CreatingMatchingBody => {
            "相符實例會在伺服器的模組下載完成後建立。屆時請再次開啟伺服器的「{more}」，選擇它，然後重新下載。"
        }
        M::CouldNotCreateInstance => "無法建立實例。",
        M::QuitGameFirst => "請先結束該實例中的遊戲。",
        M::WorldZipping => "核心正在壓縮世界…",
        M::WorldDownloading => "正在下載…",
        M::WorldDownloadingOf => "正在下載：{done} / {total}",
        M::WorldDownloadingSize => "正在下載：{done}",
        M::WorldImporting => "正在匯入…",
        M::WorldImportingOf => "正在匯入：{done} / {total}",
        M::WorldImported => {
            "已匯入為 {instance} 中的世界「{folder}」。請從「單人遊戲」開啟它；它是副本，因此變更不會同步回伺服器。"
        }
        M::PackSaved => {
            "已儲存到實例的資源包中，檔名為 {file}。在資源包清單中啟用它，也能在其他伺服器上使用。"
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Players => ["{count} 位玩家"; 2],
        P::ReadyBodySkipped => {
            ["Minecraft {version}，使用 {loader}，並包含伺服器的模組和設定檔。模組包中有 {count} 個檔案被略過。按下「開始遊戲」即可加入。";
                2]
        }
        P::ModsAdded => ["新增 {count} 個"; 2],
        P::ModsUpdated => ["更新 {count} 個"; 2],
        P::ModsRemoved => ["移除 {count} 個"; 2],
        P::OtherFiles => ["{count} 個其他檔案"; 2],
        P::ConfigFiles => ["{count} 個設定檔"; 2],
        P::ScriptFiles => ["{count} 個腳本檔案"; 2],
        P::AndMore => ["還有 {count} 個"; 2],
        P::HostDone => {
            ["{name} 已在 {core} 上，連接埠為 {port}。核心正在進行設定：它會安裝 Fabric，並從 Modrinth 下載 {count} 個模組檔案。\n\n伺服器處於停止狀態。準備好後，可以在「Lodestone Host」頁面或控制面板中啟動它；此實例已準備好加入該伺服器。";
                2]
        }
        P::HostDoneRelinked => {
            ["{name} 已在 {core} 上，連接埠為 {port}。核心正在進行設定：它會安裝 Fabric，並從 Modrinth 下載 {count} 個模組檔案。\n\n伺服器處於停止狀態。準備好後，可以在「Lodestone Host」頁面或控制面板中啟動它；此實例已準備好加入該伺服器。此實例先前連結到 {server}；現在它屬於新的伺服器。";
                2]
        }
    }
}
