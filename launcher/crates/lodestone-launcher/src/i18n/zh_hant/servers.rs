use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::FriendsPageTitle => "好友",
        M::OnThisComputer => "這台電腦上",
        M::Rescan => "重新掃描",
        M::ScanningLocal => "正在尋找這台電腦上的主機…",
        M::NoLocalCores => "這台電腦上沒有正在執行的主機。",
        M::HostOfferTitle => "架設任何伺服器，立刻和朋友一起玩",
        M::HostOfferBody => "無需設定，無需連接埠轉發。",
        M::HostOfferDownload => "下載 Lodestone Host",
        M::HostOfferCoreOnly => "僅下載伺服器程式",
        M::ConnectAsOwner => "以擁有者身分連線",
        M::ChooseDataFolder => "選擇資料夾…",
        M::ChooseDataFolderPrompt => "選擇",
        M::ProductHostHint => "Lodestone Host",
        M::ProductCoreHint => "Lodestone Core",
        M::ConnectedAsOwner => "已以擁有者身分連線",
        M::ConnectedMadeOwner => {
            "它還沒有擁有者，所以 Lodestone Launcher 為你建立了一個擁有者帳號。"
        }
        M::ConnectedOwnerKey => "此啟動器現在擁有它的擁有者金鑰。你可以在主機的控制面板中撤銷它。",
        M::OwnerAccount => "擁有者帳號",
        M::StartedBy => "啟動方式",
        M::StartedDirectly => "直接啟動",
        M::DirectNote => "此主機是直接啟動的。它應透過 lodestone_cli 執行，由其安裝並保持更新。",
        M::GetLodestoneCli => "取得 lodestone_cli",
        M::CredentialsReadFailed => "無法讀取這台電腦上主機的擁有者金鑰",
        M::CredentialsSaveFailed => "無法儲存擁有者金鑰",
        M::ClaimNoCore => "這台電腦上沒有正在執行的主機。",
        M::ClaimRefused => {
            "主機拒絕授予擁有者權限。請在它的設定中開啟「允許這台電腦上的應用程式取得擁有者權限」，或更新它。"
        }
        M::ClaimFolderNotFound => {
            "找不到此主機的資料夾。請選擇它存放資料的資料夾（它的 LODESTONE_PATH）。在 Docker 中執行的主機需要從這台電腦掛載資料夾。"
        }
        M::ClaimNotThisCore => "回應的主機並不是找到的資料夾所屬的主機，因此沒有儲存任何內容。",
        M::ClaimNoResult => "主機沒有完成認領。請再試一次。",
        M::ClaimNoCertificate => {
            "此主機沒有 HTTPS 憑證，因此 Lodestone Launcher 無法確保它的擁有者金鑰只會傳送給它。請在主機的設定中開啟 HTTPS，然後重新連線。"
        }
        M::ClaimFiles => "無法使用主機的資料夾：{error}",
        M::ClaimInvalid => "主機傳送了 Lodestone Launcher 無法理解的內容：{error}",
        M::AddServer => "新增主機",
        M::Dismiss => "關閉",
        M::OtherCoresTitle => "其他主機和伺服器",
        M::NoOtherCores => "還沒有其他主機或伺服器",
        M::NoServersHint => "透過位址新增主機，或用別人傳給你的邀請連結新增伺服器。",

        M::CoreMoreActions => "{name} 的更多動作",
        M::CopyOwnerApiKey => "複製擁有者 API 金鑰",
        M::OwnerKeyCopied => "已複製擁有者 API 金鑰",
        M::OwnerKeyCopiedHint => "請妥善保管：拿到它的人都能以擁有者身分控制這個主機。",
        M::CertificatePinned => "憑證已釘選",
        M::SignedInAs => "登入身分",
        M::CopyAddress => "複製位址",
        M::SignedIn => "已登入",
        M::NotSignedIn => "未登入：僅顯示公開伺服器",
        M::TrustedForMods => "信任此主機的模組",
        M::Offline => "離線：無法連線到此主機。請確認它正在執行，且位於同一個網路。",
        M::NotListedPublicly => {
            "擁有者尚未在核心設定中開啟「公開列出伺服器」，因此不登入就沒有可顯示的內容。"
        }
        M::NoPublicServers => "沒有公開列出的伺服器。登入後即可查看你有權存取的伺服器。",
        M::NoVisibleServers => "此主機上沒有你看得到的伺服器。",
        M::CertChangedWarning => {
            "此主機的 HTTPS 憑證在你信任之後已變更，因此 Lodestone Launcher 拒絕連線。主機會在內建憑證即將到期或位址變更時產生新憑證；但這也可能表示有人正在攔截連線。信任新憑證前，請先核對它的指紋。"
        }
        M::CertUntrustedWarning => {
            "{computer} 不信任此主機的 HTTPS 憑證。主機會自行產生憑證，因此這是正常的：請核對它的指紋，然後信任它。"
        }
        M::ReviewNewCertificate => "信任新憑證…",
        M::ReviewCertificate => "信任憑證…",

        M::CreateInstance => "建立實例",
        M::StartAndJoin => "啟動並加入",
        M::Play => "開始遊戲並加入",
        M::SharedTag => "已分享",
        M::CopyInviteLink => "複製邀請連結",
        M::InviteLinkCopied => "已複製邀請連結",
        M::InviteLinkCopiedHint => "在伺服器分享期間，拿到連結的人都能加入。",
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
        M::BusyChecking => "正在檢查伺服器的模組…",
        M::BusyUpdating => "正在更新模組…",
        M::BusyStarting => "正在啟動伺服器…",
        M::BusyStopping => "正在停止…",
        M::BusyCaption => "{stage} {progress}",

        M::ReadFailed => "無法讀取你的伺服器",
        M::SaveFailed => "無法儲存你的伺服器",
        M::NoLongerTrusted => "不再信任其模組",
        M::NoLongerTrustedIdChanged => {
            "現在由另一個主機在這個位址回應（它的 ID 已變更）。只有在你信任其擁有者時，才在它的卡片上重新信任它。"
        }
        M::NoLongerTrustedOtherCore => "現在由另一個主機在這個位址回應。",
        M::NoLongerTrustedNewCertificate => {
            "你為它信任了新的憑證。如果確定它是同一個主機，請在它的卡片上重新信任其模組。"
        }
        M::NoLongerTrustedCertificateChanged => {
            "它的憑證已變更。如果確定它是同一個主機，請在它的卡片上重新信任其模組。"
        }
        M::NotTrusted => "不受信任",
        M::SignedOut => "已登出",
        M::KeyStillValid => {
            "此裝置的金鑰在到期前仍然有效。若要立即讓它失效，請在控制面板的「設定 → API 金鑰」中撤銷它。"
        }
        M::KeySaveFailed => "無法將金鑰儲存到{keychain}",
        M::KeySaveFailedError => "無法將金鑰儲存到{keychain}：{error}",
        M::CheckCertificate => "請核對主機的憑證以繼續。",
        M::CertificateNotTrusted => "憑證未獲信任，因此你的登入資訊沒有傳送給主機。",
        M::CreateFailed => "無法建立實例",
        M::CreatedWithoutMods => "已建立，但沒有伺服器的模組",
        M::CreatedWithoutModsBody => {
            "擁有者未分享此伺服器的用戶端模組，因此這個實例只有它的 Minecraft 版本和載入器。請自行加入它需要的模組。"
        }
        M::NoServerMods => "無伺服器模組",
        M::LinkFailed => "無法將實例連結到伺服器",
        M::ReadyTitle => "實例已就緒",
        M::ReadyBody => "它包含伺服器的模組和設定檔。按下「開始遊戲」即可加入。",
        M::CheckModsFailed => "無法檢查伺服器的模組",
        M::JoinFailed => "無法加入",
        M::CoreRemoved => {
            "建立此實例時所用的主機已不在 Lodestone Launcher 中。請在「Lodestone Host」頁面重新新增它，或直接按「開始遊戲」而不加入伺服器。"
        }
        M::ServerNotShown => "主機不再向你顯示此伺服器：它已被刪除，或者它沒有公開列出而你未登入。",
        M::NotRunning => "未執行",
        M::AskOwnerToStart => "請它的擁有者啟動它，或在「Lodestone Host」頁面登入主機後自行啟動。",
        M::UpdateModsFailed => "無法更新模組",
        M::StopGameFirst => "請先結束遊戲。",
        M::FilesBusy => "請等待該實例的檔案處理完成，然後再試一次。",
        M::FinishFinderFirst => "請先完成「幫我找出來」的尋找：它關閉了部分模組。",
        M::UpdateInstanceFailed => "無法更新實例",
        M::InstanceNotSaved => "模組已更新，但無法儲存實例：{error}",
        M::BadJoinAddress => "Minecraft 無法加入此伺服器的位址或連接埠。",
        M::StartCrashed => "伺服器在啟動時當機了。請在控制面板中查看它的主控台。",
        M::StartStopped => "伺服器在啟動時停止了。請在控制面板中查看它的主控台。",
        M::ServerGone => "此伺服器已不存在。",
        M::StartTimedOut => "伺服器未能在 5 分鐘內完成啟動。",
        M::StartFailed => "無法啟動",
        M::StopFailed => "無法停止",
        M::UnlinkFailed => "無法取消連結實例：{error}",
        M::Cancelled => "已取消",
        M::TrustExplanation => {
            "來自主機的模組會以程式的形式在{computer} 上執行，擁有與你相同的檔案和帳號存取權限。只信任你信得過其擁有者的主機。"
        }
        M::FingerprintHelp => {
            "請將它與主機顯示的指紋比對：在控制面板的「設定 → 核心設定 → 資訊」中（「憑證指紋（SHA-256）」），或在主機啟動時的日誌中（「TLS certificate SHA-256 fingerprint」）。"
        }

        M::CoreUnreachable => "無法連線到主機。請檢查位址，並確認主機正在執行。（{detail}）",
        M::CoreUntrusted => {
            "{computer} 不信任 {host} 的 HTTPS 憑證。請核對它的指紋，然後信任它以進行連線。"
        }
        M::CoreCertificateChanged => {
            "{host} 的 HTTPS 憑證在你信任之後已變更，因此 Lodestone Launcher 拒絕連線。信任新憑證前，請先核對它的指紋。"
        }
        M::CoreSignedOut => "已登出：此裝置的金鑰已被撤銷或已到期。請重新登入。",
        M::CoreBadCredentials => "使用者名稱或密碼錯誤。",
        M::CoreForbidden => "主機拒絕了要求：你沒有執行此操作的權限。",
        M::CoreNotFound => "找不到",
        M::CoreTooOld => {
            "此主機的版本太舊，無法搭配 Lodestone Launcher 使用。請將它更新到支援伺服器清單的版本。"
        }
        M::CoreTooManyRequests => "嘗試次數過多。請幾分鐘後再試。",
        M::CoreAnswered => "主機回應了 {status}：{message}",
        M::CoreInvalidReply => "主機傳送了 Lodestone Launcher 無法解讀的回應：{detail}",

        M::DifferentMods => "模組與伺服器不同",
        M::DifferentModsBody => {
            "自從此實例取得伺服器的模組以來，它們已經有所變更。請先更新才能加入；你自己的模組保持不變，被取代的檔案會保存在實例的備份中。"
        }
        M::ChangeNew => "新增",
        M::ChangeChanged => "已變更",
        M::ChangeRemoved => "已移除",
        M::ChangeConfig => "設定檔和其他檔案",
        M::UpdateAndPlay => "更新並開始遊戲",
        M::PlayAnyway => "仍要開始遊戲",

        M::CertChangedTitle => "憑證已變更",
        M::CertTrustTitle => "要信任此主機的憑證嗎？",
        M::CertChangedIntro => {
            "這個主機提供的 HTTPS 憑證與你信任的不同。主機會在內建憑證即將到期或位址變更時產生新憑證，但這也可能代表有人在攔截連線。"
        }
        M::CertUntrustedIntro => {
            "{os} 不信任這個主機的 HTTPS 憑證。主機會自行產生憑證，所以第一次連線時出現這種情況是正常的。"
        }
        M::FingerprintTrustedBefore => "先前信任的憑證（SHA-256）",
        M::FingerprintPresented => "目前出示的憑證（SHA-256）",
        M::FingerprintCaption => "憑證指紋（SHA-256）",
        M::CertIfMatch => {
            "如果相符，就信任它：Lodestone Launcher 之後只接受這個憑證，並在它變更時提醒你。"
        }
        M::CertIfMatchChanged => {
            "如果相符，就信任它：Lodestone Launcher 之後只接受這個憑證，並在它變更時提醒你。已信任其模組的主機將不再受信任，直到你再次信任它。"
        }
        M::TrustedOnlyFor => "僅信任於",
        M::TrustNewCertificate => "信任新憑證",
        M::TrustCertificate => "信任憑證",

        M::TrustTitle => "信任這個主機？",
        M::TrustAfterAdd => {
            "在你信任它之前，Lodestone Launcher 會在安裝伺服器的模組前先讓你看過它們。你之後可以用主機卡片上的「{setting}」變更此設定。"
        }
        M::TrustLater => {
            "之後來自此主機的模組會直接安裝，不再詢問。你可以在主機卡片上再次關閉此設定。"
        }
        M::TrustThisCore => "信任此主機",

        M::AdvisoryTitle => "此伺服器的模組將在{computer} 上執行",
        M::AdvisoryNew => {
            "此伺服器的實例會取得這些檔案。模組是程式：它們執行時擁有與你相同的檔案和帳號存取權限。"
        }
        M::AdvisoryUpdate => {
            "更新到此伺服器的模組會安裝這些檔案。模組是程式：它們執行時擁有與你相同的檔案和帳號存取權限。"
        }
        M::ModrinthUnreachable => "無法連線到 Modrinth，因此無法核對任何檔案。",
        M::UploadedNotChecked => "上傳到伺服器，未經 Modrinth 檢查",
        M::AlsoOnModrinth => "Modrinth 上也有",
        M::ConfigFilesKey => "設定檔",
        M::ScriptFilesKey => "指令碼檔案",
        M::OtherFilesKey => "其他檔案",
        M::CanRunCode => "可執行程式碼",
        M::ScriptsNote => "指令碼檔案（KubeJS、CraftTweaker）也能驅動程式碼。",
        M::OnlyIfYouTrust => "只有在你信任主機的擁有者時才繼續。",
        M::AdvisoryTrustNote => {
            "「{button}」也會讓之後來自它的模組直接安裝，不再詢問。{explanation}"
        }
        M::InstallThisTime => "僅這次安裝",
        M::TrustAndInstall => "信任並安裝",
        M::BusyPreparing => "正在等待伺服器準備檔案…",
        M::BusyBasePack => "正在取得伺服器的整合包…",
        M::LacksFiles => "伺服器有你無法取得的檔案",
        M::LacksFilesBody => {
            "你的實例已有伺服器的檔案，但伺服器還有一些無法分享的檔案。沒有它們你可能無法加入。"
        }
        M::InstallTitle => "安裝伺服器的檔案？",
        M::InstallIntro => "新實例會取得這個遊戲版本和載入器，以及來自伺服器的這些檔案。",
        M::SummaryAddedMods => "伺服器新增的模組",
        M::SummaryServerOnly => "只有伺服器才有的檔案",
        M::MissingTitle => "伺服器無法分享的檔案",
        M::MissingNote => "你將沒有這些檔案，缺少它們可能無法加入。請詢問伺服器的擁有者。",
        M::WithheldSymlink => "符號連結",
        M::WithheldUnreadable => "無法讀取",
        M::WithheldBadName => "名稱無法分享",
        M::WithheldKeptChanging => "一直在變更",
        M::WithheldCurseForge => "CurseForge 不允許",
        M::WithheldOther => "未分享",
        M::CurseForgeTitle => "請自行從 CurseForge 下載這些",
        M::CurseForgeNote => "它們的作者不允許其他應用程式下載。",
        M::Install => "安裝",
        M::ErrUpdateLodestone => {
            "此伺服器執行的是較新版本的 Lodestone。請更新 Lodestone Launcher 後再加入。"
        }
        M::ErrPrepareFailed => "伺服器無法準備其檔案。請幾分鐘後再試。",
        M::ErrStillPreparing => "伺服器仍在準備其檔案。請幾分鐘後再試。",
        M::ErrFileFailed => "伺服器無法傳送這個檔案。請稍後重試。",
        M::ErrViewChanged => "下載期間伺服器的檔案有所變更。請再試一次。",
        M::ErrFilesChangedSince => {
            "這個檔案在計算更新後有所變更，所以沒有做任何變更。請再次按下「開始遊戲」。"
        }
        M::ErrBasePackGone => {
            "伺服器的模組包已不在 Modrinth 上，因此無法下載其檔案。請伺服器的擁有者更新模組包。"
        }
        M::ErrGameChanged => {
            "伺服器的遊戲版本已變更，此實例不再與它相符。請為該伺服器建立一個新實例。"
        }
        M::ErrCurseForgeUnreachable => {
            "無法連線到 CurseForge，而伺服器有來自它的檔案。請檢查網路連線，然後再試一次。"
        }
        M::ErrCurseForgeBase => {
            "伺服器以 CurseForge 模組包為基礎，Lodestone Launcher 暫時無法從伺服器安裝。"
        }
        M::ErrRefused => "伺服器傳送了 Lodestone Launcher 不會安裝的檔案：{detail}",
        M::ErrBaseFileMissing => {
            "伺服器的整合包中沒有這個檔案，而伺服器說它來自該整合包。請讓伺服器擁有者更新整合包。"
        }
        M::ErrOlderCore => "該主機執行的是較舊版本的 Lodestone，無法以這種方式分享其伺服器的檔案。",
        M::ErrInstallFiles => "無法安裝伺服器的檔案：{detail}",
        M::ErrStoppedSharing => {
            "伺服器已停止分享其檔案，因此無法檢查這些檔案。請詢問伺服器的擁有者。"
        }
        M::ErrNotEnoughSpace => "此磁碟上沒有足夠的可用空間來存放伺服器的檔案。",
        M::ErrNotLinked => "此實例已不再連結到該伺服器，因此沒有安裝任何內容。",

        M::KeyDays90 => "90 天",
        M::KeyYear => "1 年",
        M::KeyNever => "永不到期",
        M::AddressPlaceholder => "192.168.1.20 或 localhost:16662",
        M::UsernameOptional => "使用者名稱（選填）",
        M::Username => "使用者名稱",
        M::Password => "密碼",
        M::EnterAddress => "請輸入主機的位址。",
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
        M::AddIntro => "不登入時，你只會看到其擁有者公開列出的伺服器。",
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
        M::LinkedTo => "帳號已連結。",
        M::YourLodestoneUser => "你的 Lodestone 使用者",
        M::Connecting => "正在連線…",
        M::Live => "即時",
        M::Reconnecting => "正在重新連線：{reason}",
        M::ConsoleNotShown => "未顯示主控台",
        M::OutputAppearsHere => "伺服器的輸出會顯示在這裡。",
        M::Send => "傳送",
        M::ChatHint => "文字會透過 /say 傳給所有人。",
        M::Console => "主控台",
        M::JumpToLatest => "跳到最新",
        M::MinecraftAccount => "Minecraft 帳號",
        M::CoreTooOldToLink => "此主機的版本太舊，無法連結 Minecraft 帳號。",
        M::Unlink => "取消連結",
        M::LinkAccount => "連結",
        M::LodestoneUser => "Lodestone 使用者",
        M::LinkCheckHint => "主機會使用這次登入向 Microsoft 確認此帳號屬於你，只用一次，不會保留。",
        M::SignInMicrosoftToLink => {
            "使用 Microsoft 帳號登入，即可將它連結到你的 Lodestone 使用者。"
        }

        M::HostTitle => "在 Lodestone 上託管",
        M::ShowOnServersPage => "在「Lodestone Host」頁面查看",
        M::CoreLabel => "主機",
        M::PasswordNotKept => "你的密碼只會在登入時傳送一次，不會被儲存。",
        M::ServerName => "伺服器名稱",
        M::Memory => "記憶體",
        M::EulaAgree => "我同意 Minecraft EULA（{url}）。主機會為新伺服器接受它。",
        M::ReadEula => "閱讀 EULA",
        M::CreateServer => "建立伺服器",
        M::NameTheServer => "請為伺服器命名。",
        M::HostNeedsPassword => {
            "請輸入你在此主機上的使用者名稱和密碼。建立伺服器需要比{computer} 上的金鑰更高的權限。"
        }
        M::AgreeEulaFirst => "請先同意 Minecraft EULA：主機會為新伺服器接受它。",
        M::StageExporting => "正在匯出實例…",
        M::StageCreating => "正在建立伺服器…",
        M::CoreGone => "該主機已不存在。",
        M::InstanceGone => "該實例已不存在。",
        M::SignedOutRightAway => "主機在你登入後立刻將你登出。請再試一次。",
        M::AddCoreFirst => "請先在「Lodestone Host」頁面新增你的主機。",
        M::HostIntro => {
            "根據此實例在主機上建立伺服器：包括它的模組、設定檔以及你選擇的世界。僅玩家需要的模組、你的帳號和你自己的設定會留在這裡。伺服器建立後處於停止狀態。"
        }
        M::HostWorlds => "要傳送的世界",
        M::HostWorldsHint => "下一步選擇哪個世界成為伺服器的世界。不傳送世界則建立新世界。",
        M::HostNoWorlds => "此實例沒有世界：伺服器會建立一個新世界。",
        M::HostWorldsLoading => "正在尋找實例的世界…",
        M::HostStageSending => "正在傳送到主機…",
        M::Sent => "已傳送",
        M::HostStageReading => "主機正在讀取檔案…",
        M::HostPort => "連接埠",
        M::HostPortInvalid => "請輸入 1 到 65535 之間的連接埠。",
        M::HostNameForbidden => "伺服器名稱不能包含 / \\ : * ? \" < > |",
        M::HostNameTooLong => "伺服器名稱最多 100 個字元。",
        M::HostNameControl => "伺服器名稱不能包含控制字元。",
        M::HostCreatingHint => "你可以關閉此視窗：主機會繼續設定伺服器，準備好後此實例會連結到它。",
        M::HostCreated => {
            "它目前處於停止狀態：準備好後，請在「Lodestone Host」頁面或控制面板中啟動它。此實例已連結到它，所以「開始遊戲」會加入它。"
        }
        M::HostCreatedUnlinked => {
            "它目前處於停止狀態：準備好後，請在「Lodestone Host」頁面或控制面板中啟動它。實例在此期間已被刪除，所以沒有任何內容連結到它。"
        }
        M::HostCreatedRelinked => "此實例之前連結到另一個伺服器；現在它屬於新的伺服器。",
        M::LinkedBefore => "之前連結",
        M::HostLeftOut => "伺服器上沒有的內容",
        M::HostCreationFailed => "主機無法建立伺服器：{error}",
        M::HostCreationFailedPlain => "主機無法建立伺服器。",
        M::HostStillCreating => {
            "主機仍在設定伺服器。準備好後它會出現在「Lodestone Host」頁面上；此實例還沒有連結到它。"
        }
        M::HostTooOld => {
            "該主機的 Lodestone 版本太舊，無法從檔案建立伺服器。請在那台電腦上更新它，然後重試。"
        }
        M::HostNoPermission => "你在這個主機上的使用者無法建立伺服器。請詢問其擁有者。",
        M::HostGameRunning => "請先結束遊戲：遊戲執行時無法傳送它的世界。",
        M::DownloadWorldAction => "下載這個世界…",
        M::DownloadWorldTitle => "下載世界",
        M::DownloadWorldIntro => {
            "根據此伺服器的一個備份，在這台電腦上建立新實例：包括它的世界以及玩家取得的模組。伺服器的設定、管理員和外掛仍保留在伺服器上。"
        }
        M::DownloadWorldSignIn => "請用可以讀取此伺服器檔案的使用者登入。",
        M::DownloadWorldNeedsPassword => "請輸入你在此主機上的使用者名稱和密碼。",
        M::DownloadWorldBackups => "備份",
        M::DownloadWorldMidWrite => "在伺服器執行時備份：世界可能在寫入途中被保存。",
        M::TakenWhileRunning => "執行時備份",
        M::DownloadWorldBackUpNow => "立即備份",
        M::DownloadWorldButton => "下載",
        M::DownloadWorldBackingUp => "主機正在備份伺服器…",
        M::DownloadWorldPreparing => "主機正在準備檔案…",
        M::DownloadWorldDownloading => "正在下載…",
        M::Downloaded => "已下載",
        M::DownloadWorldTooOld => {
            "該主機的 Lodestone 版本太舊，無法傳送世界。請在那台電腦上更新它，然後重試。"
        }
        M::DownloadWorldNothingToDownload => {
            "此伺服器沒有備份，而你的使用者無法備份它。請讓它的擁有者備份後再試。"
        }
        M::DownloadWorldBackupForbidden => "你的使用者無法備份此伺服器。請改選它的一個備份。",
        M::DownloadWorldForbidden => {
            "你的使用者無法讀取此伺服器的檔案，因此無法下載它的世界。請聯絡它的擁有者。"
        }
        M::DownloadWorldNoBackups => "此伺服器還沒有備份。請先備份。",
        M::DownloadWorldBackupFailed => "備份失敗：{error}",
        M::DownloadWorldCancelled => "下載已取消。",

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
        M::AddInvite => "新增",
        M::UnnamedServer => "未命名伺服器",
        M::WaitingForReply => "等待其主機回應",
        M::AddAndJoin => "新增並加入",
        M::AlreadyAddedTitle => "已新增",
        M::AlreadyAddedBody => "此連結指向本頁上的一個伺服器，它的那一列已醒目顯示。",
        M::OnYourNetwork => "你的網路",
        M::OnYourNetworkHint => "此網路上已在區域網路上公開的世界。",
        M::JoinWorld => "加入區域網路世界",
        M::PickInstance => "選擇用於遊玩的實例。它需要與這個世界相同的 Minecraft 版本和模組。",
        M::NoInstancesYet => "你還沒有任何實例。請先建立一個。",

        M::FriendsTitle => "來自邀請連結的伺服器",
        M::JoinAFriend => "新增伺服器",
        M::SharedBy => "分享者",
        M::SharedWithFriends => "已與好友分享",
        M::SharedWithFriendsHint => "你透過它們的邀請連結加入過這些伺服器。",
        M::ConnectToJoinDirectly => "以擁有者身分連線後即可直接加入，無需通道。",
        M::DirectConnection => "在這台電腦上直接連線，無需通道",
        M::FriendCheck => "檢查",
        M::FriendChecking => "正在檢查…",
        M::FriendPlayingNow => "正在遊戲中",
        M::StopTrusting => "不再信任",
        M::ConnectionSameComputer => "同一台電腦",
        M::ConnectionLocalNetwork => "區域網路",
        M::ConnectionIpv6 => "IPv6",
        M::ConnectionInternet => "網際網路",
        M::PingMs => "{ms} 毫秒",
        M::JoinFriendTitle => "加入伺服器",
        M::AddServerTitle => "新增伺服器",
        M::JoinFriendIntro => "即使伺服器目前離線，「新增」也會儲存它。",
        M::InvitePlaceholder => "邀請連結（https://connect.lodestone.cc/j/…）",
        M::NotAnInvite => {
            "這不是 Lodestone 邀請連結。邀請連結以 https://connect.lodestone.cc/j/ 開頭。"
        }
        M::PasteInviteFirst => "請先貼上邀請連結。",
        M::ConnectingToFriend => "正在連線到好友的伺服器…",
        M::StageFinding => "正在尋找你和好友網路之間的直接連線，這需要幾秒鐘。",
        M::StageFindingSlow => "仍在尋找。某些網路最多需要一分鐘。",
        M::StageReading => "已連線。正在讀取伺服器資訊…",
        M::FriendHasMods => "此伺服器使用模組。Lodestone Launcher 會為你安裝相同的模組。",
        M::FriendNoMods => "無需模組",
        M::MadeForThisServer => "為此伺服器建立",
        M::SameVersion => "版本相同",
        M::MakesInstance => "點選「開始遊戲」會建立一個版本相同的新實例。",
        M::MakesInstanceWithMods => "點選「開始遊戲」會建立一個與伺服器版本和模組相同的新實例。",
        M::FriendServerStopped => "伺服器已停止",
        M::FriendServerStoppedBody => "請好友啟動伺服器。伺服器執行後即可開始遊戲。",
        M::FriendServerStarting => "伺服器正在啟動",
        M::FriendServerStartingBody => "伺服器執行後即可開始遊戲。",
        M::FriendStarting => "正在準備遊戲…",
        M::FriendInstanceBusy => "該實例已在執行。請先結束遊戲。",
        M::FriendLinkEnded => "好友已停止共享此伺服器",
        M::FriendLinkEndedBody => "請向好友索取新連結。",
        M::FriendNotSharing => "好友目前沒有共享此伺服器",
        M::FriendNotSharingBody => {
            "好友的電腦或 Lodestone Host 可能已關閉。請稍後再試，或請好友開始共享。"
        }
        M::FriendNoPath => "你和好友的網路無法直接連線",
        M::FriendNoPathBody => {
            "Lodestone Connect 會直接連接兩台電腦，但這兩個網路阻擋了這種連線。請嘗試換一個網路（例如手機熱點），或請好友在路由器上設定連接埠轉送。"
        }
        M::FriendWrongHost => "這不是好友的伺服器",
        M::FriendWrongHostBody => {
            "回應的電腦並不是此連結所指向的電腦，因此 Lodestone Launcher 在傳送任何內容之前就停止了。可能有人冒充好友的伺服器。請向好友索取新連結，並且只使用直接從好友那裡得到的連結。"
        }
        M::FriendOffline => "這台電腦似乎未連上網路",
        M::FriendOfflineBody => "請檢查網路連線，然後重試。",
        M::FriendServiceUnreachable => "無法連線到 Lodestone Connect",
        M::FriendServiceUnreachableBody => {
            "connect.lodestone.cc 沒有回應（{detail}）。請檢查網路連線，然後重試。"
        }
        M::FriendJoinFailed => "無法加入",
        M::FriendJoinFailedBody => "發生問題：{detail}",
        M::FriendTunnelEnded => "連線已中斷",
        M::FriendTunnelEndedBody => {
            "好友已停止共享，或者其電腦已斷線。Minecraft 會提示連線已中斷。等好友回來後再開始遊戲。"
        }
        M::FriendLinkMissing => {
            "Lodestone Launcher 無法讀取此伺服器的邀請連結。請再次開啟好友傳來的連結。"
        }
        M::FriendRemoved => {
            "此實例是為好友的伺服器建立的，但該伺服器已不在 Lodestone Launcher 中。請再次開啟其邀請連結以加入。"
        }
        M::InviteLinkKey => "邀請連結",
        M::FriendsReadFailed => "無法讀取好友的伺服器",
        M::FriendsSaveFailed => "無法儲存好友的伺服器",
        M::MagicLocalhost => "Magic localhost 位址",
        M::WhileConnected => "連線期間有效。",
        M::FriendPortTaken => "連接埠已被占用",
        M::FriendPortTakenBody => {
            "這台電腦上的其他程式正在使用常用連接埠，所以這次請在 Minecraft 中使用這個位址。下次 Lodestone Launcher 會先嘗試常用連接埠。"
        }
        M::AddressNow => "本次位址",
        M::UsualPort => "常用連接埠",
        M::EulaNotAccepted => "此伺服器尚未接受 Minecraft EULA",
        M::EulaNotAcceptedBody => {
            "在有人接受 Mojang 的 Minecraft EULA（{url}）之前，伺服器無法啟動。請先閱讀，只有在你同意時才接受。"
        }
        M::AcceptEula => "接受 Minecraft EULA",
        M::EulaAccepted => "你已為此伺服器接受 Minecraft EULA，現在可以啟動了。",
        M::EulaAcceptFailed => "無法接受 EULA：{error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Players => ["{count} 位玩家"; 2],
        P::ReadyBodySkipped => {
            ["它包含伺服器的模組和設定檔。整合包中有 {count} 個檔案被略過。按下「開始遊戲」即可加入。";
                2]
        }
        P::ReadyBodyMissing => {
            ["它包含伺服器的檔案。缺少 {count} 個伺服器無法分享的檔案。按下「開始遊戲」即可加入。";
                2]
        }
        P::AndMore => ["還有 {count} 個"; 2],
    }
}
