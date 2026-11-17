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
        M::GameVersionFollowsModpack => "隨模組包更新而變化。",
        M::GameVersionFollowsServer => "隨連結伺服器的模組包而變化。",
        M::LoaderInstallsNextStart => "{loader} 會在遊戲下次啟動時安裝。",
        M::LoaderInstallsNextPlay => "{loader} 會在你下次按下「開始遊戲」時安裝。",
        M::SavedNextStart => "已儲存。會在遊戲下次啟動時生效。",

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
        M::FromCurseForge => "來自 CurseForge",
        M::FromServerMods => "來自 {server} 的模組：「開始遊戲」會讓它們與伺服器保持一致。",
        M::FromMrpack => "來自不在 Modrinth 上的 .mrpack 檔案，因此無法在這裡更新。",
        M::ViewOnModrinth => "在 Modrinth 上查看",
        M::ViewOnCurseForge => "在 CurseForge 上查看",
        M::DownloadingNewVersion => "正在下載新版本…",
        M::CheckingModrinth => "正在檢查 Modrinth…",
        M::CheckingCurseForge => "正在檢查 CurseForge…",
        M::UpdateTo => "更新至 {version}…",
        M::UpToDate => "已是最新版本",
        M::CheckForUpdates => "檢查更新",
        M::ModrinthUnreachable => "無法連線至 Modrinth。",
        M::CurseForgeUnreachable => "無法連線至 CurseForge。",
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
        M::AzulZulu8 => "適用於 Apple 晶片的 Azul Zulu 8",
        M::MojangIntelJavaVersion => "Mojang 提供的 Intel 版 Java {major}，透過 Rosetta 2 執行",
        M::JavaTheVersionNeeds => "這個版本所需的 Java",
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
        M::NoSnapshots => "還沒有快照。",
        M::SnapshotSizes => "{total}，其中新增 {new}",
        M::SnapshotNothingNew => "{total}，無新增",
        M::SnapshotWithMods => "含模組",
        M::PreparingFile => "正在準備檔案…",
        M::DragToShare => "將檔案拖到其他 App 中，或：",
        M::SaveAsEllipsis => "另存為…",
        M::ShowInFolder => "在檔案夾中顯示",
        M::DeleteSnapshotTitle => "刪除 {name}？",
        M::DeleteSnapshotBody => {
            "快照會被刪除。它的檔案會在下次清理時刪除，除非其他快照也在使用。此動作無法復原。"
        }
        M::TakingSnapshot => "正在建立快照…",
        M::ImportingSnapshot => "正在建立實例…",
        M::ReadingSnapshot => "正在讀取快照…",
        M::ImportSnapshotTitle => "從快照新增實例",
        M::ImportServerTitle => "從伺服器的世界新增實例",
        M::CreateInstanceButton => "建立實例",
        M::SnapshotGameRunning => "請先停止遊戲。",
        M::SnapshotsHint => "此實例的世界和檔案的副本，可用於還原或分享。",
        M::SnapshotIncludeEverything => "包含模組和設定",
        M::SnapshotIncludeEverythingHint => {
            "不勾選時，快照保留世界和其他檔案，並記住它們執行時使用的模組。"
        }
        M::SnapshotAfterPlay => "每次遊戲結束後拍攝快照",
        M::SnapshotAfterPlayHint => {
            "遊玩後複製世界。跟隨「設定 › 實例預設設定」，預設關閉，除非你開啟。"
        }
        M::SnapshotAfterPlayDefaultHint => {
            "每次遊戲結束後複製世界，適用於未自行設定的實例。預設關閉，除非你開啟。"
        }
        M::SnapshotKeepHint => {
            "啟動器自動拍攝的快照會保留一段時間：最近 {last} 個、{days} 天內每天一個、{weeks} 週內每週一個。你自己拍攝的快照會一直保留，直到你刪除。"
        }
        M::BadgeAutomatic => "自動",
        M::BadgeAfterPlaying => "遊戲後",
        M::BadgeImported => "已匯入",
        M::BadgePinned => "已釘選",
        M::RestoreEllipsis => "還原…",
        M::ExportEllipsis => "匯出…",
        M::PinSnapshot => "釘選",
        M::UnpinSnapshot => "取消釘選",
        M::UnreadableImagesHint => "有無法讀取的映像時，舊的內容歷史不會被清理。",
        M::DeleteUnreadable => "刪除它們",
        M::RestoreTitle => "還原 {name}",
        M::RestoreWorldsOnly => "僅世界",
        M::RestoreWorldsAndMods => "世界及其模組",
        M::RestoreEverything => "全部",
        M::RestoreAsNew => "作為新實例",
        M::RestoreAsNewName => "新實例名稱",
        M::RestoreEverythingHint => {
            "整個遊戲資料夾還原到快照時的狀態：世界、模組、設定和選項。之後建立的世界會被刪除。"
        }
        M::WorldGoneNow => "{size} · 實例中已沒有",
        M::ModsDiffer => "這些世界上次執行時使用的是其他模組。",
        M::FilesDiffer => "這些世界上次執行時使用的是其他檔案。",
        M::RestoreModsToo => "同時還原它們的模組",
        M::RestoreModsTooChanges => "同時還原它們的模組將會：",
        M::RestoreLoaderChange => "載入器將還原為 {version}。",
        M::RestoreSafety => "會先為被取代的內容拍攝快照，並保留 30 天。",
        M::RestoreOtherGame => "此快照屬於其他 Minecraft 版本或載入器，只能作為新實例還原。",
        M::RestoreButton => "還原",
        M::RestoringSnapshot => "正在還原快照…",
        M::DeletingSnapshot => "正在刪除快照…",
        M::ChooseWorld => "請至少選擇一個世界。",
        M::ExportTitle => "匯出 {name}",
        M::ExportNote => {
            "來自 Modrinth 和 CurseForge 的模組和資源包只會列出而不會複製：匯入檔案時會在作者允許的情況下重新下載。"
        }
        M::ExportLeftOut => "以下檔案可能包含密碼和登入資訊，已排除：{files}。",
        M::ImportFrom => "{name} · Minecraft {version} · {loader}",
        M::ImportWorlds => "世界：{worlds}",
        M::ImportDownloads => "需要從 Modrinth 和 CurseForge 下載 {size}。",
        M::ImportNotDownloadable => "作者不允許下載，因此已排除：{files}。",
        M::ImportServerOnlyMods => "只在伺服器上執行的模組，已略過：{names}",
        M::ImportServerPack => {
            "只有伺服器玩家使用的檔案來自它在 Modrinth 上的模組包，可能包含模組：它們是遊戲啟動時執行的程式。只從你信任的伺服器下載世界。"
        }
        M::ImportServerPackCurseforge => {
            "伺服器的模組包來自 CurseForge，目前還不能從伺服器下載，因此只有其玩家使用的檔案不會一起帶來。"
        }
        M::ImportHostLeftOut => "主機沒有把這些放進檔案：{files}",
        M::ImportHostBlanked => "主機清空了這些密鑰：{names}",
        M::ImportSpace => "需要 {size} 磁碟空間（含 1 GB 餘量）。",
        M::ImportNeedsModrinth => "無法連線 Modrinth，無法下載檔案中的模組。",
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
        M::ExportHintBoth => "匯出為 Modrinth 的 .mrpack 檔案，或 CurseForge 的 .zip 檔案。",
        M::ExportButton => "匯出…",
        M::ExportCurseForgeButton => "匯出 CurseForge .zip…",
        M::ExportModrinthButton => "匯出 Modrinth .mrpack…",
        M::Exporting => "正在匯出…",
        M::Duplicate => "製作副本",
        M::Duplicating => "正在製作副本…",
        M::ChangeVersionOrLoader => "其他版本或載入器",
        M::ChangeVersionOrLoaderHint => {
            "實例會保留建立時的版本和載入器。副本可以使用其他版本或載入器。"
        }
        M::CopyAs => "以其他版本或載入器複製",
        M::CopyAsButton => "換版本複製…",
        M::CopyAsKeywords => "變更 切換 更新 升級 降級 版本 載入器 Fabric Forge NeoForge Quilt",
        M::CopyAsTitle => "以其他版本或載入器複製",
        M::CopyFrom => "以 {name}（{version}）為基礎新增一個實例，原實例保持不變。",
        M::CopyWhat => "要複製的內容",
        M::CopyWorlds => "世界",
        M::CopyNoWorlds => "此實例沒有世界。",
        M::CopyWorldsOlder => "世界無法複製到較舊的 Minecraft 版本：舊版本無法開啟它們。",
        M::CopyWorldsUnknown => "世界不會複製：Lodestone Launcher 無法判斷 {version} 是否較新。",
        M::CopyWorldsUpgraded => {
            "Minecraft {version} 開啟世界時會將其升級，且無法還原。原本的世界仍保留在 {name} 中。"
        }
        M::CopyWorldsModsGone => {
            "副本開啟世界時，未一同複製的模組中的方塊和物品會被移除。原本的世界仍保留在 {name} 中。"
        }
        M::CopyPacks => "資源包、光影和選項",
        M::CopyMods => "模組",
        M::CopyReading => "正在讀取實例的模組…",
        M::CopyNoMods => "此實例沒有模組。",
        M::CopyModsVanilla => "原版 Minecraft 不執行模組。",
        M::CopyModsOffline => "無法連線至 Modrinth，因此無法複製模組。",
        M::CopyModsFailed => "無法檢查模組：{error}",
        M::CopyModsNoBuild => "沒有適用於 {target} 的組建，以下模組不會複製：{names}。",
        M::CopyModsNotOnModrinth => "不在 Modrinth 上，以下模組不會複製：{names}。",
        M::CopyModsUnchecked => "無法在 Modrinth 上檢查以下模組，因此不會複製：{names}。",
        M::CopyOrUncheckMods => "或取消勾選「模組」，不帶模組複製。",
        M::CopySameTarget => {
            "請選擇其他 Minecraft 版本或載入器。若要原樣複製此實例，請使用「維護」中的「製作副本」。"
        }
        M::CopyAsStart => "開始複製",
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
        M::SnapshotsNotChanged => "無法變更快照",
        M::RestoreKeptAsideTitle => "有些檔案妨礙了還原",
        M::SnapshotRestored => "已還原「{name}」。",
        M::ImportLeftOutTitle => "部分檔案未匯入",
        M::ImportLeftOut => "這些檔案無法下載，因此新實例中沒有它們：{files}。",
        M::ImportPackSkippedTitle => "缺少模組包的檔案",
        M::ImportPackSkipped => {
            "無法下載或放置伺服器的模組包（沒有網路連線、可用空間不足，或有檔案未通過驗證），因此新實例中沒有只有其玩家使用的檔案。"
        }
        M::ImportPackSkippedGone => {
            "伺服器的模組包已不在 Modrinth 上，因此新實例中沒有只有其玩家使用的檔案。請讓伺服器擁有者更新模組包。"
        }
        M::ImportPackSkippedCurseforge => {
            "伺服器基於 CurseForge 模組包，目前還無法從伺服器安裝，因此新實例中沒有只有其玩家使用的檔案。"
        }
        M::RestoreKeptAside => "還原期間寫入的檔案保存在 {folder}。",
        M::ImportNoRoom => "磁碟空間不足：需要 {size}（含 1 GB 餘量），可用 {free}。",
        M::ImportSpaceFree => "需要 {size} 磁碟空間（含 1 GB 餘量）；可用 {free}。",
        M::ExportNotDownloadable => "這些無法下載，因此匯入該檔案的人不會得到它們：{files}。",
        M::SnapshotAfterPlayOwnHint => "遊玩後複製世界。僅為此實例設定。",
        M::DeleteUnreadableNote => "它們將被永久刪除。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShareNotShared => ["有 {count} 個檔案不會被分享"; 2],
        P::ShareAddedCount => ["加入 {count} 項"; 2],
        P::ShareRemovedCount => ["移除 {count} 項"; 2],
        P::ShareChangedCount => ["變更 {count} 項"; 2],
        P::UnreadableImages => ["{count} 個映像無法讀取"; 2],
        P::RestoreMissing => ["映像儲存遺失了其中 {count} 個檔案，無法完整還原。"; 2],
        P::ImportModsWarning => {
            ["包含 {count} 個模組：它們是遊戲啟動時執行的程式。只匯入你信任的人傳送的檔案。"; 2]
        }
        P::ShareToggledCount => ["啟用或停用 {count} 項"; 2],
        P::CodePackChanges => ["+ {count} 項變更"; 2],
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
        P::CopyWorldsCount => ["{count} 個世界"; 2],
        P::CopyCheckingMods => ["正在 Modrinth 上檢查 {count} 個模組…"; 2],
        P::CopyModsFound => ["{count} 個模組中有 {found} 個有適用於 {target} 的組建。"; 2],
        P::RestorePutsBack => ["還原 {count} 個檔案：{names}"; 2],
        P::RestoreRemoves => ["刪除 {count} 個檔案：{names}"; 2],
        P::RestoreChanges => ["變更 {count} 個檔案：{names}"; 2],
        P::ImportCarriedMods => {
            ["其中 {count} 個包含在檔案本身中，而不是從 Modrinth 或 CurseForge 下載的：{names}"; 2]
        }
        P::ImportFromServer => [
            "來自 {server} 的伺服器備份：包括它的世界和玩家會取得的模組。{count} 個伺服器檔案不會被帶入（伺服器的設定、管理員和插件）。",
            "來自 {server} 的伺服器備份：包括它的世界和玩家會取得的模組。{count} 個伺服器檔案不會被帶入（伺服器的設定、管理員和插件）。",
        ],
    }
}
