use crate::i18n::{InstanceSettingsMsg as M, InstanceSettingsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::MemoryFormat => "請以 MB（例如 4096）或 GB（例如 4G）為單位輸入記憶體，至少 256 MB。",
        M::MemoryMinAboveMax => "最小值大於最大值。",
        // 「這台 Mac」ends in Latin letters, 「這台電腦」doesn't.
        M::MemoryTooHigh => {
            if cfg!(target_os = "macos") {
                "這超過{computer} 記憶體的四分之三，留給 {os} 和其他 App 的記憶體會太少。"
            } else {
                "這超過{computer}記憶體的四分之三，留給 {os} 和其他應用程式的記憶體會太少。"
            }
        }
        M::TotalMemory => {
            if cfg!(target_os = "macos") {
                "{computer} 有 {size} 記憶體。"
            } else {
                "{computer}有 {size} 記憶體。"
            }
        }
        M::MemoryRange => "{min} – {max}",
        M::MemoryUnit => "MB",
        M::WindowDefaultSize => "遊戲預設大小",
        M::WindowFullscreen => "{size}，全螢幕",
        M::WindowSizeInvalid => "視窗大小需要以像素為單位的寬度和高度。",
        M::EnvInvalid => "環境變數：{error}",
        M::EnvPlaceholder => "名稱=值",

        M::General => "一般",
        M::GameAndLoader => "遊戲與載入器",
        M::JavaAndMemory => "Java 與記憶體",
        M::LaunchOptions => "啟動選項",
        M::SharedGameSettings => "共用遊戲設定",
        M::Maintenance => "維護",
        M::Modpack => "模組包",
        M::LinkedServer => "連結的伺服器",

        M::Name => "名稱",
        M::Group => "群組",
        M::NoGroup => "無群組",
        M::GroupHintAuto => "同一群組的實例會列在一起。沒有群組時列在「{group}」底下。",
        M::GroupHint => "同一群組的實例會列在一起。",
        M::NameMissing => "請為實例命名。",
        M::Icon => "圖示",
        M::IconHint => "顯示在側邊欄和啟動器各處。",
        M::PlayBackground => "開始遊戲頁背景",
        M::PlayBackgroundHint => {
            "顯示在「開始遊戲」分頁的名稱後方。啟動器會視圖片需要調暗文字下方的區域。"
        }
        M::UseAsIcon => "用作圖示",
        M::UseAsBackground => "用作背景",
        M::SavedSeePlayTab => "已儲存。到「開始遊戲」分頁查看。",
        M::YourScreenshots => "你的截圖",
        M::FindingScreenshots => "正在尋找你的截圖",
        M::NoScreenshots => "還沒有截圖。在遊戲中按 F2 截圖，或選擇一個檔案。",
        M::ChooseFile => "選擇檔案…",
        M::UseDefault => "使用預設",
        M::BlocksAndItems => "方塊與物品",
        M::Emoji => "表情符號",
        M::UploadImage => "上傳圖片…",

        M::AutoGroupFabric => "Fabric（模組）",
        M::AutoGroupForge => "Forge（模組）",
        M::AutoGroupNeoForge => "NeoForge（模組）",
        M::AutoGroupQuilt => "Quilt（模組）",
        M::AutoGroupVanilla => "原版",
        M::AutoGroupImported => "已匯入",

        M::GameVersionHint => "在建立實例時設定。",
        M::LoaderInstallsNextStart => "{loader} 會在遊戲下次啟動時安裝。",
        M::LoaderInstallsNextPlay => "{loader} 會在你下次按下「開始遊戲」時安裝。",
        M::SavedNextStart => "已儲存。會在遊戲下次啟動時生效。",
        M::CouldntTurnModsOff => "無法停用模組：{error}",

        M::Vanilla => "原版",
        M::ModLoader => "模組載入器",
        M::ModLoaderHint => "變更會在你下次按下「開始遊戲」時安裝。",
        M::LoaderVersion => "載入器版本",
        M::ChooseLoaderVersion => "請選擇 {loader} 版本。",
        M::ChooseVersion => "選擇版本",
        M::LoadingLoaderVersions => "正在載入 {loader} 版本",
        M::CouldntLoadVersions => "無法載入版本：{error}",
        M::CouldntLoadLoaderVersions => "無法載入 {loader} 版本：{error}",
        M::NoVersionsFor => "沒有適用於 Minecraft {game} 的版本。",
        M::LoaderHasNoVersions => "{loader} 沒有適用於 Minecraft {game} 的版本。",
        M::PlainMinecraft => "原版 Minecraft，不含模組。",
        M::ChooseGameVersionFirst => "請先選擇 Minecraft 版本。",
        M::Recommended => "推薦",
        M::Beta => "測試版",

        M::FromModrinth => "來自 Modrinth",
        M::FromServerMods => "來自 {server} 的模組：「開始遊戲」會讓它們與伺服器保持一致。",
        M::FromMrpack => "來自不在 Modrinth 上的 .mrpack 檔案，因此無法在這裡更新。",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::Updates => "更新",
        M::UpdatesHint => {
            "更新會取代模組包本身的檔案，並保留你的模組、設定檔和世界。你會先看到有哪些變更。"
        }
        M::DownloadingNewVersion => "正在下載新版本…",
        M::CheckingModrinth => "正在檢查 Modrinth…",
        M::UpdateTo => "更新至 {version}…",
        M::UpToDate => "已是最新版本",
        M::CheckForUpdates => "檢查更新",
        M::ModrinthUnreachable => "無法連線至 Modrinth。",
        M::StopGameFirst => "請先停止遊戲，或等待更新完成。",
        M::CouldntPrepareUpdate => "無法準備更新：{error}",

        M::LinkedServerHint => "位於 {core}。「開始遊戲」會加入該伺服器並讓模組保持一致。",
        M::LinkedServerCoreRemoved => {
            "位於 {core}，此核心已從「伺服器」頁面移除。請在該頁重新加入此核心以加入伺服器。"
        }
        M::CoreRemoved => "核心已移除",
        M::Unlink => "解除連結",
        M::Unlinked => "已解除連結",
        M::UnlinkNote => {
            "解除連結會保留實例及其模組。「開始遊戲」將不再加入該伺服器，也不再更新其模組。"
        }

        M::ChangeSetting => "變更",
        M::JavaMemoryLead => "標示「預設」的值來自啟動器的設定。變更其中一個值即可只為此實例設定。",
        M::Memory => "記憶體",
        M::Checking => "正在檢查…",
        M::JavaFound => "Java {major}（版本 {version}）",
        M::JavaTooOld => {
            "Java {major}（版本 {version}）。Minecraft {game} 需要 Java {needed} 或更新版本。"
        }
        M::MojangJavaVersion => "Mojang 提供的 Java {major}",
        M::MojangJava => "Mojang 提供的 Java",
        M::JavaOverrideHint => "用來取代 Mojang 所提供 Java 的 java 執行檔。",
        M::JavaHint => "Mojang 會為每個 Minecraft 版本提供合適的 Java。",
        M::UseThisJava => "使用此 Java",
        M::Check => "檢查",
        M::JvmArguments => "JVM 參數",
        M::GameArguments => "遊戲參數",
        M::NoArguments => "無",
        M::NoVariables => "無",
        M::ArgumentsHint => "以空格分隔；含空格的值請加上引號。",
        M::Window => "視窗",
        M::Fullscreen => "全螢幕",
        M::WindowHint => "大小留空即使用遊戲預設大小。",
        M::EnvironmentVariables => "環境變數",
        M::EnvHint => "名稱=值，以空格分隔。",

        M::VerifyGameFiles => "驗證遊戲檔案",
        M::VerifyHint => "檢查每個檔案，並下載遺失或損毀的檔案。",
        M::Verify => "驗證",
        M::VerifyStarted => "正在檢查每個檔案。進度會顯示在「開始遊戲」按鈕上。",
        M::Export => "匯出",
        M::ExportHint => "其他啟動器可以匯入的 .mrpack 檔案。",
        M::ExportButton => "匯出…",
        M::Exporting => "正在匯出…",
        M::Duplicate => "製作副本",
        M::DuplicateHint => "一份擁有獨立資料夾、模組和世界的副本。",
        M::HostOnCore => "在 Lodestone Core 上架設",
        M::HostHint => "用此實例的模組在你的其中一個核心上建立伺服器。",
        M::HostButton => "架設…",
        M::DeleteInstance => "刪除實例",
        M::DeleteHint => "刪除其資料夾，包括世界和模組。刪除前會先請你確認。",
        M::DeleteButton => "刪除…",

        M::SharedLead => "options.txt 中的滑鼠、按鍵、顯示、音效等設定，在每個實例中保持一致。",
        M::UseSharedSettings => "使用共用設定",
        M::SharingOff => "已關閉：此實例使用自己的 options.txt，也不會從中讀回任何內容。",
        M::SharingPreferred => "這是首選實例：你在此實例的遊戲中變更的設定會套用到每個實例。",
        M::SharingFrom => "在 {name} 的遊戲中變更的設定，會在此實例下次啟動時同步過來。",
        M::SharingNoPreferred => "沒有首選實例，因此共用值只能在啟動器的設定中變更。",
        M::Preferred => "首選",
        M::MakePreferred => "設為首選",
        M::MadePreferred => "此實例中的遊戲內變更現在會套用到每個實例。",
        M::StopGameToChange => "請先停止遊戲再變更：遊戲結束時會重寫 options.txt。",
        M::NotShared => "未共用（啟動器設定）",
        M::ShareAll => "全部共用",
        M::OverrideAll => "全部覆寫",
        M::Show => "顯示",
        M::Hide => "隱藏",
        M::NotSharedYet => "尚未共用",
        M::NotInOptions => "不在 options.txt 中",
        M::ThisInstance => "此實例",
        M::UseShared => "使用共用值（{value}）",
        M::Shared => "共用",
        M::Override => "覆寫",
        M::Value => "值",
        M::KeepForInstance => "為此實例保留",
        M::Dismiss => "略過",
        M::CategoryControls => "控制",
        M::CategoryVideo => "顯示",
        M::CategorySound => "音效",
        M::CategoryAccessibility => "協助工具與聊天",
        M::CategoryLanguage => "語言",
        M::CategoryControlsDescription => "滑鼠靈敏度、反轉、原始輸入、按鍵、蹲下與衝刺切換",
        M::CategoryVideoDescription => {
            "視野、顯示與模擬距離、介面大小、亮度、全螢幕、垂直同步、最高 FPS、粒子、畫質"
        }
        M::CategorySoundDescription => "所有音量滑桿、字幕、方向性音訊",
        M::CategoryAccessibilityDescription => "朗讀程式、文字背景、聊天大小與不透明度、效果",
        M::CategoryLanguageDescription => "遊戲語言和字型",

        M::NewInstance => "新增實例",
        M::MinecraftVersion => "Minecraft 版本",
        M::NameLabel => "名稱",
        M::MinecraftVersionLabel => "MINECRAFT 版本",
        M::ModLoaderLabel => "模組載入器",
        M::MyInstance => "我的實例",
        M::SearchVersions => "搜尋版本",
        M::Releases => "正式版",
        M::Snapshots => "快照版",
        M::OldVersions => "遠古版本",
        M::KindRelease => "正式版",
        M::KindSnapshot => "快照版",
        M::KindBeta => "Beta 版",
        M::KindAlpha => "Alpha 版",
        M::KindOther => "其他",
        M::LoadingVersionList => "正在載入 Mojang 的版本清單",
        M::NoVersionsMatch => "沒有相符的版本。",
        M::GameWithLoader => "Minecraft {version}，使用 {loader} {loader_version}",
        M::NoVersionSelected => "未選擇版本",
        M::NoModpackPicked => "未選擇模組包",
        M::PickModpack => "請選擇模組包和版本。",
        M::ChooseGameVersion => "請選擇 Minecraft 版本。",
        M::CreateInstance => "建立實例",
        M::InstallModpack => "安裝模組包",
        M::ImportMrpack => "匯入 .mrpack…",
        M::Import => "匯入",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::LoaderNextStartModsOff => {
            ["{loader} 會在遊戲下次啟動時安裝。已停用 {count} 個模組。"; 2]
        }
        P::LoaderNextPlayModsOff => {
            ["{loader} 會在你下次按下「開始遊戲」時安裝。已停用 {count} 個模組。"; 2]
        }
        P::SharedCount => ["{count} 項共用"; 2],
        P::OverriddenCount => ["{count} 項覆寫"; 2],
        P::AndMore => ["{names}及另外 {count} 項"; 2],
        P::ChangedInGame => {
            ["你在遊戲中變更的 {count} 項設定（{names}）將在下次啟動時被共用值取代。"; 2]
        }
    }
}
