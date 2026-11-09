use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "結束 {app}",
        M::Language => "語言",
        M::LanguageSystem => "系統",
        M::LanguageSystemIs => "系統（{language}）",
        M::Zoom => "縮放",
        M::ViewMenu => "顯示方式",
        M::ZoomIn => "放大",
        M::ZoomOut => "縮小",
        M::ActualSize => "實際大小",
        M::EditMenu => "編輯",
        M::Undo => "復原",
        M::Redo => "重做",
        M::Undid => "已復原：{change}",
        M::Redid => "已重做：{change}",
        M::ChangeTo => "{setting} → {value}",
        M::SettingsOf => "{name} 的設定",

        M::Play => "開始遊戲",
        M::Playing => "遊戲中",
        M::TabContent => "內容",
        M::TabWorlds => "世界",
        M::Settings => "設定",
        M::TabLogs => "日誌",

        M::LastPlayed => "最近遊玩",
        M::Instances => "實例",
        M::NoInstancesYet => "還沒有實例。",
        M::NewInstance => "新增實例…",
        M::ImportInstances => "匯入實例…",
        M::Servers => "伺服器",
        M::AppVersion => "Lodestone Launcher v{version}",
        M::PlayedInTotal => "{when} · 共 {time}",
        M::ImportProgress => "{percent} · {label}",
        M::GroupModded => "{loader}（模組）",
        M::Vanilla => "原版",
        M::GroupImported => "已匯入",

        M::NoInstanceSelected => "未選取實例",
        M::NoInstanceSelectedHint => "建立實例，即可選擇 Minecraft 版本和載入器。",
        M::MinecraftVersion => "Minecraft {version}",
        M::MinecraftWith => "Minecraft {version}，{loader}",
        M::HeroPlayed => "{game}。已遊玩 {time}。",
        M::HeroNotPlayed => "{game}。按下「開始遊戲」即可下載並啟動遊戲。",
        M::CreateFirstInstance => "建立你的第一個實例",
        M::InstanceExplainer => "實例是一個 Minecraft 版本，擁有自己的模組、世界和設定。",
        M::ContentPlaceholder => "有了實例後，內容會顯示在這裡。",
        M::WorldsPlaceholder => "有了實例後，世界會顯示在這裡。",
        M::SettingsPlaceholder => "有了實例後，設定會顯示在這裡。",
        M::LogsPlaceholder => "有了實例後，日誌會顯示在這裡。",
        M::LinkedTo => "已連結到 {core} 上的 {server}",
        M::LinkedToRemoved => "已連結到 {core} 上的 {server}（該核心已從「伺服器」頁面移除）",
        M::PickerInstance => "實例",
        M::PickerVersion => "版本",
        M::NoInstances => "沒有實例",
        M::AndJoin => "並加入 {server}",
        M::PlayWithoutJoining => "開始遊戲但不加入",

        M::DeleteNamed => "刪除「{name}」？",
        M::DeleteWorldBody => "這個世界的資料夾會移到{trash}，你可以從那裡還原。它的備份會保留。",
        M::DeleteInstanceBody => {
            "這個實例的資料夾（包括其中的世界、模組和設定）會移到{trash}。你可以從那裡還原。"
        }
        M::ReplaceWorldTitle => "用這個備份取代「{name}」？",
        M::ReplaceWorldBody => {
            "「{name}」會回到{when} 時的狀態。之後的所有變更都會從世界中遺失，但會先備份，目前的資料夾會移到{trash}。"
        }
        M::BackUpAndReplace => "備份並取代",
        M::SwitchToLoader => "切換到 {loader}？",
        M::SwitchToVanilla => "切換到原版？",
        M::ModsWontLoadOn => "以下已啟用的模組是為其他載入器製作的，在 {loader} 上無法載入：",
        M::ModsWontLoadVanilla => "原版 Minecraft 不會載入任何模組，因此以下已啟用的模組無法載入：",
        M::ModMadeFor => "{loader} 模組",
        M::TurningOffRenames => {
            "停用後，它們會被重新命名為 .disabled，之後你可以在「內容」中重新啟用。"
        }
        M::WorldsAtRisk => "使用過這些模組中的方塊或物品的世界可能會失去它們。請先備份你的世界。",
        M::TurnOffAndSave => "停用並儲存",
        M::SaveAndKeep => "儲存並保留",
        M::RemoveAccountTitle => "移除「{name}」？",
        M::RemoveAccountBody => {
            "Lodestone Launcher 會忘記這個帳號，並從你的{keychain}中刪除它儲存的登入資訊。之後你可以再次新增。"
        }
        M::RemoveAccount => "移除帳號",
        M::MinecraftCrashed => "Minecraft 已當機：{description}。",
        M::MinecraftExitCode => "Minecraft 已停止，結束代碼為 {code}。",
        M::MinecraftStopped => "Minecraft 意外停止。",
        M::CrashTitle => "「{name}」已當機",
        M::LikelyCause => "可能的原因",
        M::OpenCrashReport => "開啟當機報告",
        M::CauseWrongJava => {
            "Java 版本不符。這個 Minecraft 版本（或某個模組）需要其他版本的 Java。請在「設定」中清除自訂 Java，以使用 Mojang 提供的 Java。"
        }
        M::CauseOutOfMemory => "記憶體不足。請在「設定」中提高記憶體上限，或關閉其他應用程式。",
        M::CauseNatives => {
            "原生程式庫遺失，或與處理器架構不符。在 Apple 晶片上，1.19 之前的版本會透過 Rosetta 2 在 Intel 版 Java 中執行；請確認已安裝 Rosetta，且自訂 Java 不是 arm64 版。"
        }
        M::CauseJvmOptions => "Java 拒絕了其中一項設定。請檢查「設定」中的 JVM 參數和記憶體。",
        M::CauseGraphics => {
            if cfg!(target_os = "macos") {
                "遊戲無法建立視窗或初始化 OpenGL。請更新 macOS，或嘗試不使用自訂視窗大小或全螢幕。"
            } else {
                "遊戲無法建立視窗或初始化 OpenGL。請更新顯示卡驅動程式，或嘗試不使用自訂視窗大小或全螢幕。"
            }
        }
        M::CopyLog => "複製日誌",

        M::UpdateTitle => "更新「{name}」？",
        M::ModpackSummary => "{pack} {from} → {to}。Minecraft {game}，{loader}。需下載 {size}。",
        M::NoFilesChange => "沒有檔案變更，只會記錄新版本。",
        M::UpdatedFiles => "已更新（{count}）",
        M::AddedFiles => "已新增（{count}）",
        M::RemovedFiles => "已移除（{count}）",
        M::ReplacedOwnFiles => "原本屬於你、現由模組包提供的檔案（{count}）",
        M::KeptFiles => "保留你的變更（{count}）",
        M::WrittenFiles => "寫入的設定檔和其他檔案（{count}）",
        M::ModpackUpdateSafety => {
            "你自己的模組、世界和變更過的設定檔不會受到影響。每個被取代或移除的檔案都會先儲存到實例的備份資料夾。"
        }
        M::BackUpEveryWorld => "先備份所有世界",

        M::SaveChangesTitle => "要儲存變更嗎？",
        M::UnsavedDefaultsBody => "你變更了實例預設設定。要儲存還是捨棄？",
        M::UnsavedInstanceBody => "你變更了「{name}」的設定。要儲存還是捨棄？",
        M::UnsavedThisInstanceBody => "你變更了這個實例的設定。要儲存還是捨棄？",
        M::UnsavedInDefaults => "實例預設設定中有尚未儲存的變更",

        M::GroupLauncher => "啟動器",
        M::InstanceDefaults => "實例預設設定",
        M::SharedGameSettings => "共用遊戲設定",
        M::General => "一般",
        M::Accounts => "帳號",
        M::Developer => "開發者",
        M::DefaultsLead => "每個實例都可以在自己的設定中變更。",
        M::SharedLead => "所有實例使用相同的遊戲選項。",
        M::NoAccount => "沒有帳號",
        M::ManageAccounts => "管理帳號",
        M::GroupAutomatically => "自動將實例分組",
        M::GroupAutomaticallyHint => "依載入器為未分組的實例分組。",
        M::OfflineAccount => "離線帳號",
        M::MicrosoftAccount => "Microsoft 帳號",
        M::SignInAgain => "重新登入",
        M::InUse => "使用中",
        M::Use => "使用",
        M::AddAnAccount => "新增帳號",
        M::AddMicrosoftEllipsis => "Microsoft 帳號…",
        M::AddOfflineEllipsis => "離線帳號…",

        M::Memory => "記憶體",
        M::MemoryError => "記憶體：{error}",
        M::MemoryDefaultHint => "預設：{min} – {max}",
        M::MegabytesUnit => "MB",
        M::Window => "視窗",
        M::Fullscreen => "全螢幕",
        M::JvmArguments => "JVM 參數",
        M::GameArguments => "遊戲參數",
        M::EnvironmentVariables => "環境變數",

        M::SyncedFrom => "現在，在「{name}」遊戲中變更的設定會更新所有實例。",
        M::SyncedFromNone => "現在沒有實例會更新共用設定，請在這裡編輯。",
        M::SharedCleared => "共用值已清除。各實例保留自己的 options.txt。",
        M::NoInstance => "沒有實例",
        M::EditHereOnly => "只在這裡編輯",
        M::Value => "值",
        M::SyncFrom => "同步來源",
        M::SyncFromHint => "在此實例遊戲內的變更會同步到其他實例。",
        M::SyncFromNoneHint => "共用值只在這裡變更。",
        M::StartFromInstance => "從某個實例開始",
        M::CopyFromInstance => "從實例複製…",
        M::ClearShared => "清除共用值",
        M::ClearSharedHint => "各實例保留自己的 options.txt。",
        M::ClearEveryShared => "清除所有共用值",
        M::ClearEllipsis => "清除…",
        M::NoValuesYet => "還沒有值",
        M::Hide => "隱藏",
        M::Show => "顯示",
        M::ServerList => "伺服器清單",
        M::ServerListHint => "即將推出。",
        M::Source => "來源",
        M::WhatsShared => "共用內容",
        M::WhatsSharedLead => "關閉的類別仍由各實例個別設定。",
        M::CategoryControls => "控制",
        M::CategoryVideo => "顯示",
        M::CategorySound => "音效",
        M::CategoryAccessibility => "協助工具與聊天",
        M::CategoryLanguage => "語言",
        M::CategoryControlsHint => "滑鼠、按鍵",
        M::CategoryVideoHint => "視角廣度、顯示距離、介面大小、畫質",
        M::CategorySoundHint => "音量、字幕",
        M::CategoryAccessibilityHint => "朗讀功能、聊天、文字背景",
        M::CategoryLanguageHint => "遊戲語言和字型",
        M::NarratorSection => "朗讀功能",
        M::KeepNarratorOff => "保持關閉朗讀功能",
        M::KeepNarratorOffHint => "同時略過首次啟動時的協助工具畫面。",
        M::NarratorNotShared => "「{category}」未共用，因此每個實例保留自己的朗讀功能設定。",
        M::NarratorKeptOff => "朗讀功能在每個實例中都保持關閉，包括新實例。",
        M::NarratorPerInstance => "各實例恢復使用自己的朗讀功能設定。",

        M::SigningIn => "正在登入…",
        M::OfflineCouldntConnect => "離線 – 無法連線",
        M::AddMinecraftAccount => "新增 Minecraft 帳號",
        M::Offline => "離線",
        M::AddMicrosoftAccount => "新增 Microsoft 帳號",
        M::AddOfflineAccount => "新增離線帳號",
        M::RemoveNamed => "移除「{name}」",
        M::OfflineNote => "離線 – 僅限單人遊戲和離線模式伺服器",

        M::OfflineDialogBody => {
            "用於開發和測試。這個帳號沒有 Microsoft 登入，它的 UUID 與離線模式伺服器為這個名稱指派的相同。"
        }
        M::Username => "使用者名稱",
        M::UsernameRule => "3 到 16 個字母、數字或底線。",
        M::AddAccount => "新增帳號",

        M::LauncherIdentity => "啟動器身分",
        M::SignInAsLauncher => "以 Minecraft Launcher 身分登入",
        M::SignInAsLauncherHint => "在 Lodestone 自己的應用程式獲准前使用。僅供測試。",
        M::SignInAsLauncherTitle => "以 Minecraft Launcher 身分登入？",
        M::WhatThisDoes => "這個選項的作用",
        M::WhatThisDoesBody => {
            "Lodestone Launcher 會使用 Mojang 官方 Minecraft Launcher 的公開用戶端 ID，向 Microsoft 宣稱自己就是該啟動器，而不是使用 Lodestone 自己註冊的應用程式。Microsoft 和 Mojang 並未核准這種做法，Lodestone 與它們沒有關聯，而且這可能違反 Microsoft 對其應用程式的條款。"
        }
        M::Risks => "風險",
        M::RisksBody => {
            "Microsoft 隨時可能封鎖這種做法。一旦封鎖，以這種方式登入的帳號將無法使用。在 Lodestone 的應用程式獲得核准前，請僅用於個人測試。"
        }
        M::WhyHere => "為什麼提供這個選項",
        M::WhyHereBody => {
            "Lodestone 自己的應用程式仍在等待 Mojang 核准，因此一般的 Microsoft 登入暫時無法完成。"
        }
        M::WhenSignIn => "登入時",
        M::WhenSignInBody => {
            "「{add}」會給你一組代碼。在 microsoft.com/link 輸入代碼，並在那裡登入你的 Microsoft 帳號。Microsoft 的頁面可能會顯示 Minecraft 的名稱，而不是 Lodestone 的。如果代碼無效，你也可以改用連結登入：瀏覽器會開啟 login.live.com，你需要將它最後停留的空白頁面網址貼回來。"
        }
        M::WhatLauncherGets => "啟動器會取得什麼",
        M::WhatLauncherGetsBody => {
            "與一般登入相同：讓它能以該帳號玩 Minecraft 的權杖。長期有效的權杖儲存在你的{keychain}中，其餘的只儲存在記憶體中。這些帳號會在帳號選單中標示為「{badge}」。"
        }
        M::HowToUndo => "如何復原",
        M::HowToUndoBody => {
            "關閉這個設定，並將這些帳號登出。關閉時，Lodestone Launcher 會詢問是否要將它們登出。"
        }
        M::TurnItOn => "我了解，開啟",
        M::TurnOffLauncherTitle => "關閉以 Minecraft Launcher 身分登入？",
        M::TurnOffLauncherBody => "新的 Microsoft 登入會重新使用 Lodestone 自己的應用程式。",
        M::OneLauncherAccount => "「{name}」正以 Minecraft Launcher 身分登入。",
        M::TurnOffLauncherAccounts => {
            "{accounts}保留的帳號會繼續可用，直到 Microsoft 封鎖這種做法。登出會將它們從 Lodestone Launcher 移除，並從你的{keychain}中刪除它們儲存的登入資訊。"
        }
        M::TurnOffAndSignOut => "關閉並登出",
        M::TurnOffOnly => "只關閉",

        M::SignInWithMicrosoft => "使用 Microsoft 登入",
        M::LauncherIdentityNote => {
            "開發者選項：以 Minecraft Launcher 身分登入，而不是以 Lodestone Launcher 身分。Microsoft 的頁面可能會顯示 Minecraft 的名稱，而不是 Lodestone 的。"
        }
        M::UseLinkInstead => "改用登入連結",
        M::UseCodeInstead => "改用代碼",
        M::SignInCodeBody => {
            "使用你玩 Minecraft: Java Edition 的 Microsoft 帳號登入。你會取得一組代碼，需要在 Microsoft 網站上輸入。"
        }
        M::SignInBrowserBody => {
            "使用你玩 Minecraft: Java Edition 的 Microsoft 帳號登入。瀏覽器會開啟以完成登入。"
        }
        M::KeepsSignIn => "Lodestone Launcher 會將你的登入資訊儲存在{keychain}中。",
        M::GetCode => "取得代碼",
        M::WaitingForBrowser => "正在等待瀏覽器…",
        M::FinishInBrowser => "請在開啟的瀏覽器視窗中完成登入，然後回到這裡。",
        M::OpenBrowserAgain => "重新開啟瀏覽器",
        M::GettingCode => "正在取得代碼…",
        M::GoToHost => "在任何裝置上前往 {host} 並輸入這組代碼：",
        M::CodeCopiedWaiting => "已複製代碼。正在等待你輸入…",
        M::WaitingForCode => "正在等待你輸入代碼…",
        M::CopyCodeAndOpen => "複製代碼並開啟 {host}",
        M::LinkInstructions => {
            "請在開啟的瀏覽器視窗中登入。接著 Microsoft 會顯示一個頁面，說明這個頁面通常不會顯示。從瀏覽器的網址列複製完整網址，並且只貼到這個啟動器中。"
        }
        M::LinkStartsWith => "網址以 {url} 開頭",
        M::SigningInToMinecraft => "正在登入 Minecraft…",
        M::ExpectedUntilApproved => {
            "在 Mojang 核准這個應用程式之前，這是正常的。你的帳號沒有任何問題。"
        }

        M::AuthCancelled => "登入已取消",
        M::AuthCancelledBody => "登入已取消。",
        M::AuthTimedOut => "登入逾時",
        M::AuthTimedOutBody => "瀏覽器登入未能及時完成。請再試一次，或改用代碼。",
        M::AuthStateMismatch => "登入要求不相符",
        M::AuthStateMismatchBody => {
            "瀏覽器傳回了另一個登入要求，因此 Lodestone Launcher 忽略了它。請再試一次。"
        }
        M::AuthDeclined => "登入遭拒",
        M::AuthDeclinedBody => "登入在瀏覽器中遭到拒絕。請再試一次並接受所需權限以繼續。",
        M::AuthMicrosoft => "Microsoft 登入失敗",
        M::AuthMicrosoftBody => "Microsoft 無法讓你登入（{code}）。請稍後再試。",
        M::AuthMicrosoftSaid => "Microsoft 回應：{description}（{code}）",
        M::AuthCodeExpired => "代碼已過期",
        M::AuthCodeExpiredBody => "代碼在登入完成前已過期。請取得新代碼後再試一次。",
        M::AuthSignInExpiredBody => {
            "你儲存的 Microsoft 登入已過期或遭撤銷。請重新登入以繼續使用這個帳號。"
        }
        M::AuthNoXbox => "沒有 Xbox 個人檔案",
        M::AuthNoXboxBody => {
            "這個 Microsoft 帳號還沒有 Xbox 個人檔案。請先在 xbox.com 登入一次以建立，然後再試一次。"
        }
        M::AuthRegion => "無法使用 Xbox Live",
        M::AuthRegionBody => "這個帳號所在的國家或地區無法使用 Xbox Live，因此無法登入 Minecraft。",
        M::AuthAdult => "需要成人驗證",
        M::AuthAdultBody => "這個帳號需要先在 Xbox 網站上完成成人驗證才能遊玩。這是韓國的規定。",
        M::AuthChild => "兒童帳號",
        M::AuthChildBody => "這是兒童帳號。需要由成人將它加入 Microsoft 家庭群組後才能登入。",
        M::AuthXbox => "Xbox Live 登入失敗",
        M::AuthXboxBody => "Xbox Live 拒絕了登入（錯誤 {code}）。請稍後再試。",
        M::AuthNotApproved => "尚未獲得 Minecraft 核准",
        M::AuthNotApprovedBody => {
            "Microsoft 登入正常，但 Minecraft 尚未核准 Lodestone Launcher。目前請使用離線模式。"
        }
        M::AuthNotOwned => "找不到 Minecraft",
        M::AuthNotOwnedBody => {
            "這個 Microsoft 帳號沒有 Minecraft: Java Edition。如果你透過 Xbox Game Pass 遊玩，請先在官方啟動器中啟動一次 Minecraft，然後再試一次。"
        }
        M::AuthNoProfile => "沒有 Minecraft 個人檔案",
        M::AuthNoProfileBody => {
            "這個帳號還沒有 Minecraft 個人檔案。如果你透過 Xbox Game Pass 遊玩，請先在官方啟動器中啟動一次 Minecraft 並選擇名稱，然後再試一次。"
        }
        M::AuthRateLimited => "嘗試次數過多",
        M::AuthRateLimitedBody => "登入嘗試次數過多。請等候幾分鐘後再試一次。",
        M::AuthNetwork => "無法連線",
        M::AuthNetworkBody => "Lodestone Launcher 無法連線到登入伺服器。請檢查網路連線後再試一次。",
        M::AuthUnexpected => "登入失敗",
        M::AuthUnexpectedBody => "「{step}」過程中發生問題（HTTP {status}）。請稍後再試。",
        M::AuthListener => "無法開始登入",
        M::AuthListenerBody => {
            "Lodestone Launcher 無法開啟瀏覽器返回時使用的本機頁面。請改用代碼。"
        }
        M::AuthKeychain => "無法儲存登入資訊",
        M::AuthKeychainBody => {
            "Lodestone Launcher 無法將登入資訊儲存到你的{keychain}。請再試一次。"
        }
        M::AuthInvalidLink => "網址不正確",
        M::AuthNotAUrlBody => "這不是網址。請在登入後從瀏覽器的網址列複製完整網址。",
        M::AuthWrongPageBody => {
            "只接受以 {url} 開頭的網址。請在登入後，從 Microsoft 顯示的空白頁面網址列中複製。"
        }
        M::AuthNoCodeBody => "這個網址中沒有登入代碼。請先完成登入，再複製最後停留頁面的網址。",
        M::StepMicrosoftToken => "Microsoft 登入",
        M::StepDeviceCode => "Microsoft 裝置代碼登入",
        M::StepXboxLive => "Xbox Live 登入",
        M::StepXsts => "Xbox Live 授權",
        M::StepMinecraftLogin => "Minecraft 登入",
        M::StepEntitlements => "Minecraft 擁有權檢查",
        M::StepProfile => "載入 Minecraft 個人檔案",

        M::AccountsTooNew => {
            "你的帳號由較新版本的 Lodestone Launcher 儲存。在這裡所做的變更將不會儲存。"
        }
        M::AccountsCorrupt => {
            "無法讀取 accounts.json，因此 Lodestone Launcher 在沒有帳號的情況下啟動。"
        }
        M::AccountsCorruptMoved => {
            "無法讀取 accounts.json，因此 Lodestone Launcher 在沒有帳號的情況下啟動。舊檔案位於 {path}。"
        }
        M::CouldntReadAccounts => "無法讀取你的帳號：{error}",
        M::CouldntSaveSettings => "無法儲存你的設定：{error}",
        M::CouldntSaveAccounts => "無法儲存你的帳號：{error}",
        M::CouldntRemoveSignIn => "無法移除「{name}」儲存的登入資訊：{error}",
        M::AddAccountFirst => "請先新增帳號：使用 Microsoft 登入，或新增離線帳號。",
        M::SignInExpired => "請重新登入「{name}」：它儲存的登入資訊已過期。",
        M::LauncherClosing => "啟動器正在關閉。",

        M::UpdatesSection => "更新",
        M::UpdateAutomatically => "自動更新",
        M::UpdateAutomaticallyHint => "在背景從 GitHub 下載新組建。",
        M::BuildNumber => "組建 {build}",
        M::BuildNumberCommit => "組建 {build} · {commit}",
        M::DevelopmentBuild => "開發組建",
        M::UpdatesUnavailable => "此組建未在 GitHub 上發布，因此不會自動更新。",
        M::UpdateNotChecked => "每隔幾小時檢查一次 GitHub 上是否有新組建。",
        M::UpdateChecking => "正在檢查新組建…",
        M::UpToDate => "目前已是最新組建。",
        M::UpdateAvailable => "組建 {build} 現已可用。",
        M::UpdateDownloading => "正在下載組建 {build}… {percent}",
        M::UpdateReady => "組建 {build} 已安裝，重新啟動後即可使用。",
        M::UpdateReadyPlaying => "組建 {build} 已安裝，關閉遊戲後即可重新啟動。",
        M::UpdateFailed => "無法更新：{error}",
        M::CheckForUpdates => "立即檢查",
        M::DownloadUpdate => "下載",
        M::RestartToUpdate => "重新啟動",
        M::UpdateReadyBadge => "更新已就緒",
        M::UpdateAvailableBadge => "有可用的更新",
        M::MinimizeWindow => "最小化",
        M::MaximizeWindow => "最大化",
        M::RestoreWindow => "還原",

        M::RestOnModrinth => "其餘說明請在 Modrinth 上查看。",
        M::Storage => "儲存空間",
        M::Skin => "外觀主題",
        M::SkinGallery => "外觀主題",
        M::SkinImport => "匯入…",
        M::SkinImportButton => "匯入",
        M::SkinExport => "匯出…",
        M::SkinNew => "新增外觀主題…",
        M::SkinDelete => "刪除",
        M::SkinDeleteConfirm => "刪除 {name}",
        M::SkinCopyName => "{name} 副本",
        M::SkinCopied => "已複製 {name} 以便修改。",
        M::SkinImported => "已匯入 {name}。",
        M::SkinExported => "已匯出到 {path}。",
        M::SkinSafeMode => "已在不使用外觀主題的情況下啟動（--no-skin）。",
        M::SkinCouldntLoad => "無法載入外觀主題：{error}",
        M::SkinSidebar => "側邊欄",
        M::SkinImage => "圖片",
        M::SkinChoose => "選擇…",
        M::SkinUseImage => "使用圖片",
        M::SkinRemove => "移除",
        M::SkinFit => "填滿方式",
        M::SkinFitCover => "填滿",
        M::SkinFitTile => "並排",
        M::SkinTint => "色調",
        M::SkinTintStrength => "色調強度",
        M::SkinBlur => "模糊",
        M::SkinBanner => "實例橫幅",
        M::SkinBannerInSidebar => "顯示在側邊欄項目後方",
        M::SkinHero => "「開始遊戲」橫幅",
        M::SkinHeroHint => "用於沒有自己圖片的實例。",
        M::SkinPlayButton => "「開始遊戲」按鈕",
        M::SkinColor => "顏色",
        M::SkinFadeTo => "漸層到",
        M::SkinShape => "形狀",
        M::SkinShapeRounded => "圓角",
        M::SkinShapePill => "膠囊",
        M::SkinShapePixel => "像素",
        M::SkinLabel => "文字",
        M::SkinGlow => "光暈",
        M::SkinColors => "顏色",
        M::SkinAccent => "強調色",
        M::SkinProgress => "進度列",
        M::SkinDefault => "預設",
        M::SkinNone => "無",
        M::SkinAdjusted => "強調色已調整以保持可讀。",
        M::SkinReset => "還原為預設",
        M::SkinPlayColor => "「開始遊戲」按鈕顏色",
        M::SkinUseSkins => "跟隨外觀主題",
        M::SkinSurfaceWindow => "視窗",
        M::SkinSurfaceTopBar => "頂欄",
        M::SkinSurfaceSidebar => "側邊欄",
        M::SkinSurfaceMain => "頁面",
        M::SkinSurfaceHero => "遊玩圖片",
        M::SkinSurfacePlayBar => "遊玩列",
        M::SkinSurfaceCard => "卡片",
        M::SkinSurfaceSettingCard => "設定群組",
        M::SkinSurfaceRow => "清單列",
        M::SkinSurfaceDialog => "對話框",
        M::SkinSurfaceSettingsNav => "設定頁面清單",
        M::SkinSurfacePopover => "選單",
        M::SkinSurfaceInput => "文字欄位",
        M::SkinSurfaceButtonPrimary => "主要按鈕",
        M::SkinSurfaceButtonSecondary => "次要按鈕",
        M::SkinSurfaceButtonGhost => "低調按鈕",
        M::SkinSurfaceButtonDanger => "危險按鈕",
        M::SkinSurfaceToggle => "開關",
        M::SkinSurfaceTab => "分頁",
        M::SkinSurfacePill => "膠囊選擇器",
        M::SkinSurfacePlayButton => "遊玩按鈕",
        M::SkinSurfaceProgress => "進度列",
        M::SkinSurfaceToast => "通知",
        M::SkinRoleText => "文字",
        M::SkinRoleMuted => "次要文字",
        M::SkinRoleFaint => "提示與圖示",
        M::SkinRoleLabel => "標籤",
        M::SkinCheckShort => "{surface}，{role}：{ratio}:1，需要 {target}:1",
        M::SkinCheckNoFix => "在此填色上沒有文字顏色能達到；請更換填色。",
        M::SkinApplyFix => "套用建議的修正",
        M::SkinFixAll => "全部修正",
        M::SkinChecksPass => "所有文字都清晰可讀（{count} 項檢查）。",
        M::SkinChecksNote => "可讀性由外觀主題決定：以下僅為建議。",
        M::SkinBanners => "來自外觀主題",
        M::SkinBannerMissing => "此橫幅所屬的外觀主題已不存在，正在顯示預設橫幅。",
        M::SkinEditorTitle => "外觀主題編輯器",
        M::SkinPopOut => "開啟編輯器視窗",
        M::SkinEditorLead => {
            "在編輯器中修改每個介面區域、字型和橫幅。修改會立即顯示在視窗中，可以用復原還原。"
        }
        M::SkinSummary => "已設定 {surfaces} 個區域 · {fonts} 種字型 · {banners} 個橫幅",
        M::SkinSurfaces => "介面區域",
        M::SkinTokens => "文字顏色",
        M::SkinFonts => "字型",
        M::SkinBannerSet => "橫幅",
        M::SkinPlayButtonSection => "遊玩按鈕",
        M::SkinReadability => "可讀性",
        M::SkinFill => "填色",
        M::SkinFillColour => "顏色",
        M::SkinFillGradient => "漸層",
        M::SkinOpacity => "不透明度",
        M::SkinGradientLinear => "線性",
        M::SkinGradientRadial => "放射",
        M::SkinAngle => "角度",
        M::SkinStops => "顏色",
        M::SkinStopAt => "位置",
        M::SkinAddStop => "新增顏色",
        M::SkinRemoveStop => "移除顏色",
        M::SkinBuiltinImages => "內建圖片",
        M::SkinBorder => "邊框",
        M::SkinBorderWidth => "邊框寬度",
        M::SkinRadius => "圓角",
        M::SkinTextColour => "文字",
        M::SkinMutedColour => "次要文字",
        M::SkinTextElsewhere => "此區域的文字使用文字顏色設定。",
        M::SkinTokenText => "文字",
        M::SkinTokenSecondary => "標籤",
        M::SkinTokenMuted => "描述",
        M::SkinTokenFaint => "提示",
        M::SkinTokenBorder => "線條",
        M::SkinFontHeading => "標題",
        M::SkinFontBody => "內文",
        M::SkinFontSidebar => "側邊欄",
        M::SkinFontButton => "按鈕",
        M::SkinFontPlay => "遊玩按鈕和標題",
        M::SkinFontTab => "分頁",
        M::SkinFontMono => "程式碼和記錄",
        M::SkinFontAdd => "新增字型檔…",
        M::SkinBannerAdd => "新增橫幅…",
        M::SkinBannersNone => "還沒有橫幅。新實例使用預設的遊玩圖片。",
        M::SkinBannersLead => "新實例會隨機獲得一個；之後可在遊玩背景中更換。",
        M::SkinSaveAsNew => "另存為新外觀主題",
        M::SkinUseLaunchers => "使用啟動器預設",
        M::SkinPreviewIn => "在視窗中顯示",
        M::SkinPreviewSettings => "設定",
        M::SkinPreviewMenu => "選單",
        M::SkinPreviewNotice => "通知",
        M::SkinPreviewNoticeText => "通知看起來是這樣的。",
        M::SkinSampleText => "範例文字",
        M::SkinSampleMuted => "次要文字",
        M::SkinScrim => "標題下的陰影",
        M::StorageInstances => "實例",
        M::StorageGameFiles => "遊戲檔案",
        M::StorageRuntimes => "Java 執行環境",
        M::StorageCache => "快取",
        M::StorageLeftovers => "未完成的下載和匯入",
        M::StorageCleanLeftoversTitle => "移除未完成的下載？",
        M::StorageCleanLeftoversBody => {
            "這將釋放 {size}。這些是未完成的模組包下載和匯入留下的檔案。"
        }
        M::StorageOther => "設定和其他檔案",
        M::StorageCacheAndLogs => "快取和日誌",
        M::StorageLogs => "日誌和當機報告",
        M::StorageOldLogs => "{size} 超過 30 天",
        M::StorageVersions => "版本",
        M::StorageLibraries => "程式庫",
        M::StorageAssets => "資源",
        M::StorageUnused => "沒有實例使用",
        M::StorageUnusedTag => "未使用",
        M::StorageJava => "Java {major}",
        M::StorageWorlds => "世界 {size}",
        M::StorageContent => "模組和資源包 {size}",
        M::StorageOtherFiles => "其他 {size}",
        M::StorageNoInstances => "沒有實例",
        M::StorageNoRuntimes => "沒有 Java 執行環境",
        M::StorageCounting => "正在統計…",
        M::StorageCouldntCount => "無法統計檔案：{error}",
        M::StorageStopGame => "請先停止遊戲，再移除遊戲檔案、Java 執行環境或快取。",
        M::StorageOpenFolder => "開啟資料夾",
        M::StorageOpen => "開啟",
        M::StorageDelete => "刪除…",
        M::StorageRemoveEllipsis => "移除…",
        M::StorageClearEllipsis => "清除…",
        M::StorageRemove => "移除",
        M::StorageClear => "清除",
        M::StorageDeleteOld => "刪除舊的…",
        M::StorageDeleteAll => "全部刪除…",
        M::StorageDeleteConfirm => "刪除",
        M::StorageFreed => "已釋放 {size}",
        M::StorageCouldntRemove => "無法全部移除：{error}",
        M::StorageCleanGameFilesTitle => "移除未使用的遊戲檔案？",
        M::StorageCleanGameFilesBody => {
            "這將釋放 {size}。沒有實例使用的版本、程式庫和資源會被刪除，實例需要時會重新下載。"
        }
        M::StorageCleanRuntimesTitle => "移除未使用的 Java 執行環境？",
        M::StorageCleanRuntimesBody => "這將釋放 {size}。Minecraft 版本需要時會重新下載。",
        M::StorageClearCacheTitle => "清除快取？",
        M::StorageClearCacheBody => "這將釋放 {size}。啟動器會重新下載所需內容。",
        M::StorageDeleteOldLogsTitle => "刪除舊日誌？",
        M::StorageDeleteOldLogsBody => "這將釋放 {size}。超過 30 天的日誌和當機報告會被永久刪除。",
        M::StorageDeleteAllLogsTitle => "刪除所有日誌？",
        M::StorageDeleteAllLogsBody => {
            "這將釋放 {size}。所有日誌和當機報告都會被永久刪除，正在執行的遊戲的除外。"
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AndMore => ["還有 {count} 個"; 2],
        P::Values => ["{count} 個值"; 2],
        P::CopiedValues => ["已從「{name}」複製 {count} 個值。"; 2],
        P::LauncherAccounts => ["有 {count} 個帳號正以 Minecraft Launcher 身分登入：{names}。"; 2],
    }
}
