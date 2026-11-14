use crate::i18n::{InstanceSettingsMsg as M, InstanceSettingsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::MemoryFormat => "請以 MB（例如 4096）或 GB（例如 4G）為單位輸入記憶體，至少 256 MB。",
        M::MemoryMinAboveMax => "最小值大於最大值。",
        // 「這台 Mac」ends in Latin letters, 「這台電腦」doesn't.
        M::MemoryTooHigh => {
            if cfg!(target_os = "macos") {
                "這超過{computer} 記憶體的四分之三，留給 {os} 和其他應用程式的記憶體會太少。"
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
        M::MemoryChosenAutomatically => {
            "依這台電腦和實例的模組選擇。在你變更之前，它會隨模組調整。"
        }
        M::WindowDefaultSize => "遊戲預設大小",
        M::WindowFullscreen => "{size}，全螢幕",
        M::WindowSizeInvalid => "視窗大小需要以像素為單位的寬度和高度。",
        M::WindowSize => "視窗大小",
        M::WindowSizeGameDefault => "遊戲預設",
        M::WindowSizeThisScreen => "此螢幕",
        M::WindowSizeCustom => "自訂",
        M::WindowWidth => "視窗寬度",
        M::WindowHeight => "視窗高度",
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
        M::NameMissing => "請為實例命名。",
        M::Icon => "圖示",
        M::PlayBackground => "開始遊戲頁背景",
        M::UseAsIcon => "用作圖示",
        M::UseAsBackground => "用作背景",
        M::SavedSeePlayTab => "已儲存。到「開始遊戲」分頁查看。",
        M::YourScreenshots => "你的截圖",
        M::FindingScreenshots => "正在尋找你的截圖",
        M::NoScreenshots => "還沒有截圖。在遊戲中按 F2 截圖，或選擇一個檔案。",
        M::ChooseFile => "選擇檔案…",
        M::UseDefault => "使用預設",
        M::BackgroundDefault => "預設",
        M::BackgroundImage => "圖片",
        M::BackgroundSlideshow => "投影片",
        M::SlideshowThisInstance => "此實例",
        M::SlideshowAllInstances => "所有實例",
        M::SlideshowOwnImages => "我的圖片",
        M::AddImages => "新增圖片…",
        M::AddToSlideshow => "加入投影片",
        M::SlideshowNoScreenshots => "還沒有截圖。在有截圖之前顯示預設插圖。",
        M::SlideshowNoImages => "還沒有圖片。新增一些，或在下方選擇截圖。",
        M::BlocksAndItems => "方塊與物品",
        M::Emoji => "表情符號",
        M::UploadImage => "上傳圖片…",

        M::AutoGroupServers => "伺服器",
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
        M::AboutVanilla => "Mojang 發布的原版 Minecraft。",
        M::AboutFabric => "輕量、更新快。大多數效能模組。",
        M::AboutQuilt => "相容 Fabric，並有自己的擴充。",
        M::AboutForge => "經典之選。大型內容模組和舊整合包。",
        M::AboutNeoForge => "Forge 的後繼者，適用於新版本。",
        M::PrereleaseUpdates => "測試版和早期測試版更新",
        M::PrereleaseUpdatesHint => "關閉時，已在使用測試版的模組仍會收到測試版更新。",
        M::LoaderVersion => "載入器版本",
        M::ChooseLoaderVersion => "請選擇 {loader} 版本。",
        M::ChooseVersion => "選擇版本",
        M::LoadingLoaderVersions => "正在載入 {loader} 版本",
        M::CouldntLoadVersions => "無法載入版本：{error}",
        M::NoVersionsFor => "沒有適用於 Minecraft {game} 的版本。",
        M::Recommended => "推薦",
        M::Beta => "測試版",

        M::FromModrinth => "來自 Modrinth",
        M::FromServerMods => "來自 {server} 的模組：「開始遊戲」會讓它們與伺服器保持一致。",
        M::FromMrpack => "來自不在 Modrinth 上的 .mrpack 檔案，因此無法在這裡更新。",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::DownloadingNewVersion => "正在下載新版本…",
        M::CheckingModrinth => "正在檢查 Modrinth…",
        M::UpdateTo => "更新至 {version}…",
        M::UpToDate => "已是最新版本",
        M::CheckForUpdates => "檢查更新",
        M::ModrinthUnreachable => "無法連線至 Modrinth。",
        M::StopGameFirst => "請先停止遊戲，或等待更新完成。",
        M::CouldntPrepareUpdate => "無法準備更新：{error}",

        M::LinkedServerHint => "位於 {core}。「開始遊戲」會加入它並同步模組。",
        M::LinkedServerCoreRemoved => {
            "位於 {core}，此主機已從「Lodestone Host」頁面移除。請在該頁再次新增此主機，才能加入伺服器。"
        }
        M::CoreRemoved => "主機已移除",
        M::Unlink => "解除連結",
        M::Unlinked => "已解除連結",
        M::UnlinkNote => "「開始遊戲」將不再加入該伺服器，模組會保留。",

        M::ChangeSetting => "變更",
        M::JavaMemoryLead => "預設值來自啟動器設定。",
        M::Memory => "記憶體",
        M::Checking => "正在檢查…",
        M::JavaFound => "Java {major}（版本 {version}）",
        M::JavaTooOld => {
            "Java {major}（版本 {version}）。Minecraft {game} 需要 Java {needed} 或更新版本。"
        }
        M::MojangJavaVersion => "Mojang 提供的 Java {major}",
        M::MojangJava => "Mojang 提供的 Java",
        M::UseThisJava => "使用此 Java",
        M::Check => "檢查",
        M::JvmArguments => "JVM 參數",
        M::GameArguments => "遊戲參數",
        M::NoArguments => "無",
        M::NoVariables => "無",
        M::Window => "視窗",
        M::Fullscreen => "全螢幕",
        M::EnvironmentVariables => "環境變數",

        M::InstanceSnapshots => "快照",
        M::TakeSnapshotEllipsis => "建立快照…",
        M::TakeSnapshotTitle => "建立快照",
        M::TakeSnapshot => "建立快照",
        M::SnapshotName => "名稱",
        M::WhatToInclude => "包含的內容",
        M::WhatToRestore => "要還原的內容",
        M::PartMods => "模組",
        M::PartResourcePacks => "資源包",
        M::PartShaderPacks => "光影包",
        M::PartConfigs => "設定檔和選項",
        M::PartWorlds => "世界",
        M::PartScreenshots => "截圖",
        M::PartSettings => "實例設定",
        M::PartOther => "其他檔案",
        M::NoSnapshots => "還沒有快照。",
        M::SnapshotSizes => "{total}，其中新增 {new}",
        M::BeforeGoingBack => "回復到「{name}」之前",
        M::RevertEllipsis => "還原…",
        M::RevertTitle => "還原到 {name}？",
        M::RevertBody => "所選部分會被快照中的內容取代。取代前會先為它們建立快照，以便還原。",
        M::Revert => "還原",
        M::ShareEllipsis => "分享…",
        M::ShareSnapshotTitle => "分享 {name}",
        M::PreparingFile => "正在準備檔案…",
        M::DragToShare => "將檔案拖到其他 App 中，或：",
        M::SaveAsEllipsis => "另存為…",
        M::ShowInFolder => "在檔案夾中顯示",
        M::DeleteSnapshotTitle => "刪除 {name}？",
        M::DeleteSnapshotBody => "沒有其他快照使用的檔案會被刪除。此動作無法復原。",
        M::TakingSnapshot => "正在建立快照…",
        M::RevertingSnapshot => "正在還原快照…",
        M::ImportingSnapshot => "正在建立實例…",
        M::ReadingSnapshot => "正在讀取快照…",
        M::ImportSnapshotTitle => "從快照新增實例",
        M::SnapshotFrom => "{name} · Minecraft {version}",
        M::SnapshotModsWarning => {
            "其中包含 {count} 個模組：它們是遊戲啟動時會執行的程式。只使用來自你信任的人的快照。"
        }
        M::SnapshotLaunchSettingsWarning => {
            "它的實例設定會改變遊戲的啟動方式：Java、Java 參數、環境變數或記憶體。這些設定也能執行程式，所以只有選擇了「實例設定」才會使用它們。只為來自你信任的人的快照選擇它。"
        }
        M::CreateInstanceButton => "建立實例",
        M::SnapshotGameRunning => "請先停止遊戲。",
        M::ChooseSomething => "請至少選擇一項。",
        M::SnapshotTaken => "快照已建立。",
        M::SnapshotReverted => "快照已還原。",
        M::SnapshotsHint => "此實例的副本，可用於還原或分享。",
        M::VerifyGameFiles => "驗證遊戲檔案",
        M::VerifyHint => "重新下載缺少或損毀的檔案。",
        M::Verify => "驗證",
        M::VerifyStarted => "正在檢查每個檔案。進度會顯示在「開始遊戲」按鈕上。",
        M::FindCrashingMod => "尋找導致遊戲當機的模組",
        M::FindCrashingModHint => "多次啟動遊戲，每次關閉一半模組，直到只剩一個。",
        M::FindCrashingModButton => "尋找…",
        M::FindCrashingModKeywords => "當機 崩潰 衝突 損壞 不相容 排查",
        M::Export => "匯出",
        M::ExportHint => "匯出為 .mrpack 檔案。",
        M::ExportButton => "匯出…",
        M::Exporting => "正在匯出…",
        M::Duplicate => "製作副本",
        M::Duplicating => "正在製作副本…",
        M::HostOnCore => "在 Lodestone 上架設",
        M::HostHint => "用此實例的模組建立伺服器。",
        M::HostButton => "架設…",
        M::ShareCodeRow => "分享碼",
        M::ShareButton => "分享…",
        M::ShareTitle => "分享 {name}",
        M::ShareIntro => {
            "另一個 Lodestone Launcher 可以用這個分享碼建立同樣的實例：相同的 Minecraft 版本、載入器和 Modrinth 內容。"
        }
        M::ShareReading => "正在讀取內容…",
        M::ShareNotSharedBody => "分享碼只包含來自 Modrinth 的內容。如有需要，請另行傳送這些檔案。",
        M::ShareFromCurseForge => "來自 CurseForge",
        M::ShareAddedByHand => "手動加入",
        M::ShareNotOnModrinth => "不在 Modrinth 上",
        M::ShareChangedSincePack => "自模組包安裝後已變更",
        M::ShareUnchecked => "未向 Modrinth 核對",
        M::ShareBasedOnPack => "基於模組包 {pack}",
        M::ShareIntroPack => {
            "另一個 Lodestone Launcher 可以用這個分享碼建立同樣的實例：先安裝模組包，再套用你的變更。"
        }
        M::ShareNotSharedBodyPack => {
            "模組包內附的檔案會隨之而來，但這些檔案不會原樣送達。如有需要，請另行傳送。"
        }
        M::ShareNoChanges => "未變更模組包",
        M::ShareChanges => "你的變更：{changes}",
        M::CodePackPreview => "來自 Modrinth 的 {pack}",
        M::ShareMore => "還有 {count} 個",
        M::CopyCode => "複製分享碼",
        M::CodeCopied => "已複製",
        M::ShareCodeMode => "分享碼",
        M::PasteShareCode => "貼上分享碼",
        M::CodeNotACode => "這不是 Lodestone 分享碼。",
        M::CodeNewer => "此分享碼來自較新版本的 Lodestone Launcher。",
        M::CodeTooLarge => "分享碼太長。",
        M::CodeDamaged => "分享碼已損毀，請重新複製。",
        M::DeleteInstance => "刪除實例",
        M::DeleteHint => "包括其中的世界和模組。",
        M::DeleteButton => "刪除…",
        M::SharedLead => "讓各實例的設定保持同步。",
        M::UseSharedSettings => "使用共用設定",
        M::SharingOff => "此實例保留自己的選項。",
        M::SharingFrom => "接收來自 {name} 的遊戲內變更。",
        M::SharingMain => "此實例中遊戲內的變更會同步到所有實例。",
        M::SharingNoMain => "沒有主實例：共用值只能在設定中變更。",
        M::MainInstance => "主實例",
        M::MainInstanceHint => {
            "你在主實例遊戲中變更的設定，會在其他共用設定的實例下次啟動時複製過去。只能有一個主實例。"
        }
        M::IsMainInstance => "這是你的主實例",
        M::MakeMain => "設為主實例",
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
        M::Dismiss => "忽略",
        M::SharedNotApplied => "未能套用共用遊戲設定：{error}",
        M::SharedNotReadBack => "未能讀回共用遊戲設定：{error}",
        M::CategoryControls => "控制",
        M::CategoryVideo => "顯示",
        M::CategorySound => "音效",
        M::CategoryAccessibility => "協助工具與聊天",
        M::CategoryLanguage => "語言",
        M::CategoryControlsDescription => "滑鼠、按鍵",
        M::CategoryVideoDescription => "視角廣度、顯示距離、介面大小、畫質",
        M::CategorySoundDescription => "音量、字幕",
        M::CategoryAccessibilityDescription => "朗讀功能、聊天、文字背景",
        M::CategoryLanguageDescription => "遊戲語言和字型",

        M::NewInstance => "新增實例",
        M::MinecraftVersion => "Minecraft 版本",
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
        M::PickModpack => "請選擇模組包和版本。",
        M::ChooseGameVersion => "請選擇 Minecraft 版本。",
        M::CreateInstance => "建立實例",
        M::InstallModpack => "安裝模組包",
        M::ImportMrpack => "匯入 .mrpack…",
        M::Import => "匯入",
        M::MemoryKeywords => "RAM 記憶體 Xmx Xms",
        M::JavaKeywords => "JDK JRE 執行環境 路徑",
        M::JvmArgumentsKeywords => "Java 參數 選項",
        M::WindowKeywords => "解析度 寬度 高度 全螢幕 尺寸",
        M::EnvironmentKeywords => "env 環境變數 PATH",
        M::PlayBackgroundKeywords => "桌布 圖片 背景 幻燈片",
        M::DeleteKeywords => "刪除 移除 解除安裝",
        M::LoaderKeywords => "模組載入器 Fabric Forge NeoForge Quilt",

        M::AddModpackTitle => "將模組包加入 {name}",
        M::AddModpackFilter => "適用於 Minecraft {version}（{loader}）的模組包。{note}",
        M::AddModpackServerNote => {
            "模組包的模組會與 {server} 的模組放在一起，伺服器的模組保持不變，因此你仍然可以加入。"
        }
        M::AddModpackNote => {
            "模組包的檔案會與實例自己的檔案放在一起，不會取代實例中已有的任何檔案。"
        }
        M::AddModpackButton => "加入模組包",
        M::CheckingModpack => "正在檢查模組包",
        M::AddModpackSummary => "{pack} {version}：需下載 {size}。",
        M::AddModpackNothing => "此實例已包含此模組包的所有檔案。",
        M::AddGroupFiles => "將加入（{count}）",
        M::AddGroupConfigs => "將加入的設定檔（{count}）",
        M::AddGroupServer => "保持 {server} 的版本（{count}）",
        M::AddGroupPack => "保持實例模組包的版本（{count}）",
        M::AddGroupThere => "實例中已有（{count}）",
        M::AddedModpacks => "已加入的模組包",
        M::AddModpackRow => "加入模組包",
        M::AddModpackHint => "將模組包的模組和設定檔加入此實例，與已有的放在一起。",
        M::AddModpackServerHint => {
            "將模組包的模組（例如用戶端最佳化）加入到 {server} 的模組旁邊。伺服器的模組保持不變。"
        }
        M::AddModpackEllipsis => "加入模組包…",
        M::RemoveAddedModpackNote => "移除此模組包加入的檔案。你修改過的檔案會保留。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShareNotShared => ["有 {count} 個檔案不會被分享"; 2],
        P::ShareAddedCount => ["加入 {count} 項"; 2],
        P::ShareRemovedCount => ["移除 {count} 項"; 2],
        P::ShareChangedCount => ["變更 {count} 項"; 2],
        P::ShareToggledCount => ["啟用或停用 {count} 項"; 2],
        P::CodePackChanges => ["+ {count} 項變更"; 2],
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
        P::SharedWritten => ["共用遊戲設定：已將 {count} 個值寫入 options.txt"; 2],
        P::SharedWrittenNewFile => ["共用遊戲設定：已將 {count} 個值寫入新建的 options.txt"; 2],
        P::SharedEmitted => ["共用遊戲設定：此實例的 {count} 個值現已套用到所有實例"; 2],
        P::SharedChangedNote => {
            ["共用遊戲設定：遊戲中變更的 {count} 項設定將在下次啟動時被共用值取代（請見「設定」）";
                2]
        }

        P::AndMoreFiles => ["以及另外 {count} 個"; 2],
        P::AddedModpackFiles => ["已加入 {count} 個檔案"; 2],
    }
}
