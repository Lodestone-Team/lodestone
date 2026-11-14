use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::TwoSentences => "{first}{second}",
        M::ShareCodeReady => "{name} 已就緒",
        M::ShareCodeNotInstalled => "未安裝：{names}。",
        M::ShareCodeContentFailed => "無法安裝分享碼中的內容",
        M::ShareCodeOffline => "無法連線到 Modrinth，因此建立的實例不含其內容。",
        M::TitleAndMessage => "{title}：{message}",
        M::Vanilla => "原版",

        M::StagePreparing => "正在準備",
        M::StageJava => "正在下載 Java",
        M::StageLibraries => "正在下載程式庫",
        M::StageLoader => "正在安裝模組載入器",
        M::StageProcessors => "正在為模組載入器修補 Minecraft",
        M::StageAssets => "正在下載遊戲資源",
        M::StageNatives => "正在解壓縮原生程式庫",
        M::StageStarting => "正在啟動 Java",
        M::StageLaunching => "正在啟動 Minecraft",
        M::ProgressDetail => "{files_done}/{files_total} · {done} MB/{total} MB",

        M::PreparingNote => "正在準備 Minecraft {version}（{loader}）",
        M::JoiningNote => "遊戲啟動後將加入 {address}",
        M::VerifyingNote => "正在驗證 Minecraft {version}（{loader}）的遊戲檔案",
        M::RosettaNote => "遊戲透過 Rosetta 2 在 Intel（x86_64）Java 中執行。",
        M::StartingJavaNote => "正在啟動 Java：{command}",
        M::AutoMemoryNote => "自動設定：依這台電腦和實例的模組，將記憶體設為 {memory}",
        M::AutoMemoryLimitedNote => {
            "這些模組需要 {wanted} 記憶體，超出這台電腦可分配的量；遊戲時請關閉其他應用程式。"
        }
        M::AutoJvmArgsNote => "自動設定：已針對模組遊戲調整 Java 記憶體回收器",
        M::AutoLowMemoryNote => {
            "此實例有 {have} 記憶體；依它的模組，建議使用 {recommended}。可在實例的設定中變更。"
        }
        M::AutoVideoNote => "自動設定：已為這台電腦選擇視訊設定（顯示距離 {distance}）",
        M::AutoVideoNoDistanceNote => "自動設定：已為這台電腦選擇視訊設定",
        M::AutoControlsNote => "自動設定：已從 {source} 沿用滑鼠靈敏度、按鍵綁定和語言",
        M::AutoControlsLauncherNote => {
            "自動設定：已從 Minecraft 啟動器沿用滑鼠靈敏度、按鍵綁定和語言"
        }
        M::AutoControlsOtherLauncherNote => {
            "自動設定：已從 {launcher} 的 {instance} 沿用滑鼠靈敏度、按鍵綁定和語言"
        }
        M::StoppedNote => "已停止",
        M::ExitedWithCode => "Minecraft 已結束，結束代碼為 {code}",
        M::EndedBySignal => "Minecraft 因訊號而終止",
        M::CancellingNote => "正在取消…",
        M::StoppingNote => "正在停止 Minecraft",
        M::Cancelled => "已取消",

        M::CouldntSignIn => "無法登入",
        M::RosettaNeeded => "需要 Rosetta 2",
        M::RosettaMissing => {
            "此 Minecraft 版本需要 Intel（x86_64）Java，它在 Apple Silicon 上透過 Rosetta 2 執行。尚未安裝 Rosetta。請在「終端機」中執行下列指令來安裝，然後再按一次「開始遊戲」：\n\nsoftwareupdate --install-rosetta"
        }
        M::RosettaNoticeTitle => "Minecraft {version} 透過 Rosetta 2 執行",
        M::RosettaNoticeBody => {
            "Mojang 沒有為這個版本需要的部分程式庫製作 Apple 晶片版本，因此它會透過 Rosetta 2 在 Intel 版 Java 中執行。啟動會比較慢，之後可以正常遊玩。Minecraft 1.14 及更新版本會原生執行。"
        }
        M::RosettaNoticeBodyOverride => {
            "這個實例使用的是在其「設定」中選擇的 Intel 版 Java，因此遊戲會透過 Rosetta 2 執行。啟動會比較慢，之後可以正常遊玩。若要原生執行，請選擇適用於 Apple 晶片的 Java，或 Mojang 提供的 Java。"
        }
        M::NoNativeJava8 => {
            "這個 Minecraft 版本需要 Java 8，但目前沒有適用於 Apple 晶片的 Java 8：Mojang 沒有提供，Azul 也沒有列出。請稍後再試，或在實例的「設定」中選擇一個適用於 Apple 晶片（arm64）的 Java 8。"
        }
        M::NeedsIntelJava => {
            "在 Apple 晶片上，Minecraft {version} 只能透過 Rosetta 2 在 Intel（x86_64）Java 中執行，而在實例「設定」中選擇的 Java 是 Apple 晶片版。請在那裡選擇 Intel 版 Java，或 Mojang 提供的 Java。"
        }
        M::CouldntStartMinecraft => "無法啟動 Minecraft",
        M::CouldntStartJava => "無法啟動 Java",
        M::CouldntVerify => "無法驗證遊戲檔案",
        M::GameFilesVerified => "遊戲檔案已驗證",
        M::EveryFileIntact => "所有檔案皆完好無缺。{summary}",
        M::ModpackUpdating => "模組包正在更新",
        M::WaitForUpdate => "請等待更新完成，再按「開始遊戲」。",
        M::StopBeforeDeleting => "請先停止遊戲，再刪除該實例。",
        M::FilesBusy => "請等待該實例的檔案處理完成，然後再試一次。",
        M::CouldntSavePlayed => "無法儲存上次遊玩時間：{error}",
        M::CouldntSavePlayTime => "無法儲存遊戲時數：{error}",
        M::ModpackExported => "模組包已匯出",
        M::CouldntExportModpack => "無法匯出模組包",
        M::CouldntSwitchLoader => "無法切換至模組載入器",
        M::NoModLoaderFor => {
            "Lodestone Launcher 支援的模組載入器都無法在 Minecraft {version} 上執行：Fabric 從 1.14 起才支援，而 Forge 沒有適用於該版本的組建。"
        }
        M::SwitchedTo => "已切換至 {loader}",
        M::SwitchedToMessage => {
            "{name} 現在使用 {loader} {version}。下次按「開始遊戲」時會自動安裝。"
        }

        M::SearchLog => "搜尋日誌",
        M::LevelDebug => "偵錯",
        M::LevelInfo => "資訊",
        M::LevelWarn => "警告",
        M::LevelError => "錯誤",
        M::LogEmpty => "按下「開始遊戲」後，遊戲輸出會顯示在這裡。",
        M::NoLinesMatch => "沒有符合篩選條件的行。",
        M::JumpToLatest => "跳至最新",
        M::CopyLog => "複製日誌",
        M::LogSource => "要顯示的日誌",
        M::LogThisSession => "本次執行",
        M::LogNoFiles => "還沒有先前的日誌或當機報告。",
        M::LogKindLog => "日誌",
        M::LogKindCrash => "當機報告",
        M::LogKindJvm => "Java 當機",
        M::LogReading => "正在讀取 {name}",
        M::LogReadFailed => "無法讀取 {name}：{error}",
        M::LogFileEmpty => "此檔案是空的。",

        M::ImportTitle => "從其他啟動器匯入",
        M::ImportFromHere => "從這裡匯入",
        M::LinksLeftOut => "已略過指向遊戲資料夾以外的符號連結。",
        M::NewestRelease => "最新正式版",
        M::NewestSnapshot => "最新快照版",
        M::NoInstances => "沒有實例。",
        M::LookingForLaunchers => "正在尋找其他啟動器",
        M::NoLaunchersFound => {
            "在常用位置找不到其他啟動器。請選擇某個啟動器或實例的資料夾，從中匯入。"
        }
        M::Waiting => "等待中",
        M::FindingVersion => "正在確認版本和載入器",
        M::Imported => "已匯入",
        M::ImportedNote => "已匯入。{note}",
        M::Copying => "正在複製 {done} / {total}",
        M::MatchingModrinth => "正在將模組和資源包與 Modrinth 比對",
        M::CheckToImport => "勾選要匯入的實例。其檔案會被複製，原啟動器中的檔案維持不變。",
        M::LeaveWorldsOut => "不匯入世界",
        M::LeaveWorldsOutSize => "不匯入世界（{size}）",
        M::Import => "匯入",
        M::ChooseFolder => "選擇資料夾…",
        M::StopImportingTitle => "停止匯入？",
        M::StopImportingBody => "尚未完成的匯入會停止，已複製的內容會被刪除。已匯入的實例會保留。",
        M::StopImporting => "停止匯入",
        M::KeepImporting => "繼續匯入",

        M::NotAModpack => "不是模組包",
        M::NotMrpack => {
            "{name} 不是模組包。Modrinth 模組包的副檔名為 .mrpack，CurseForge 模組包是含有 manifest.json 的 .zip 檔案。"
        }
        M::CurseForgeUnreachable => {
            "這是 CurseForge 模組包，而此 Lodestone Launcher 無法連線至 CurseForge。"
        }
        M::CurseForgePackOnWebsite => {
            "此模組包只能從 CurseForge 網站下載。請在網站上下載，然後將 .zip 拖曳到 Lodestone Launcher 上。"
        }
        M::Modpack => "模組包",
        M::ReadingModpack => "正在讀取模組包",
        M::ModpackReady => "{name} 已準備就緒",
        M::ModpackReadyMessage => {
            "Minecraft {version}，使用 {loader}。按「開始遊戲」即可下載並啟動遊戲。"
        }
        M::CouldntImportModpack => "無法匯入模組包",
        M::CouldntUpdateModpack => "無法更新模組包",
        M::StopGameFirst => "請先停止遊戲。",
        M::ModpackUpdated => "模組包已更新",
        M::ModpackUpdatedMessage => {
            "{name} 現在執行 {pack} {version}。被取代的檔案已儲存在 {path}。"
        }
        M::UpdatedButNotSaved => "檔案已更新，但無法儲存實例：{error}",
        M::DownloadingName => "正在下載 {name}",
        M::InstallingName => "正在安裝 {name}",
        M::CopyingConfigs => "正在複製模組包的設定檔",
        M::RecordingFiles => "正在記錄模組包的檔案",
        M::SavingSnapshot => "正在儲存實例檔案的快照",
        M::BackingUp => "正在備份 {name}",
        M::UpdatingTo => "正在更新至 {version}",

        M::SearchModpacks => "在 Modrinth 上搜尋模組包，或貼上 Modrinth 連結",
        M::SearchModpacksCurseForge => "在 CurseForge 上搜尋模組包",
        M::SortRelevance => "相關性",
        M::SortDownloads => "下載次數",
        M::SortFollows => "追蹤數",
        M::SortUpdated => "最近更新",
        M::LinkNotModpack => "此連結指向的是{kind}，不是模組包。請在實例的「內容」分頁中加入。",
        M::KindMod => "模組",
        M::KindResourcePack => "資源包",
        M::KindShaderPack => "光影包",
        M::KindDataPack => "資料包",
        M::KindPlugin => "外掛程式",
        M::SearchingModrinth => "正在搜尋 Modrinth",
        M::NoModpacksMatch => "沒有相符的模組包。",
        M::ByAuthor => "作者 {author} · {downloads} 次下載",
        M::PickModpack => "選擇一個模組包以查看其版本。",
        M::LoadingModpack => "正在載入模組包",
        M::Versions => "版本",
        M::Description => "簡介",
        M::NoVersions => "此模組包沒有任何版本。",
        M::LoadingVersions => "正在載入版本",
        M::FindModButton => "幫我找出來",
        M::FinderTitle => "找出造成「{name}」當機的模組",
        M::FinderIntro => {
            "啟動器會多次啟動遊戲，每次停用可能造成當機的模組中的一半，直到只剩下一個。完成後，你的模組會恢復原狀。"
        }
        M::FinderAutomatic => {
            "遊戲在啟動時當機，所以你什麼都不用做。每次嘗試會在遊戲當機或進入主選單時結束。"
        }
        M::FinderManual => {
            "遊戲是在你遊玩時當機的，所以每次嘗試都需要你來玩。重複之前造成當機的操作。如果再次當機，下一次嘗試會自動開始；如果沒有，請按「沒有當機」。"
        }
        M::FinderByHand => {
            "玩每一次嘗試，做之前讓遊戲當機的操作。如果當機，下一次嘗試會自動開始；如果沒有，請按「沒有當機」。"
        }
        M::FinderStart => "開始",
        M::FinderReading => "正在檢查你的模組",
        M::FinderTry => "第 {number} 次嘗試",
        M::FinderStarting => "正在啟動遊戲",
        M::FinderWaiting => "正在等待，看遊戲是否當機",
        M::FinderSettling => "已進入主選單，正在確認遊戲不會關閉",
        M::FinderPlay => "一直玩到當機為止。如果沒有當機，請按「沒有當機」。",
        M::FinderAsk => "遊戲已關閉。問題出現了嗎？",
        M::FinderDidntCrash => "沒有當機",
        M::FinderItCrashed => "當機了",
        M::FinderStopping => "正在停止遊戲",
        M::FinderFound => "找到了",
        M::FinderFoundOne => "停用 {name} 後，遊戲不會當機。",
        M::FinderFoundGroup => {
            "停用這些模組後，遊戲不會當機：{names}。它們彼此相依，只能一起停用。"
        }
        M::FinderAlsoOff => "相依於它的模組也會保持停用：{names}。",
        M::FinderKeepOff => "保持停用",
        M::FinderPutBack => "全部重新啟用",
        M::FinderNotFound => {
            "即使停用了這些模組，遊戲仍然當機，所以原因可能不是模組，或者當機只是偶爾發生。你的模組已恢復原狀。"
        }
        M::FinderRefused => {
            "有些模組少了被停用的模組就無法載入，所以只能停止尋找。你的模組已恢復原狀。"
        }
        M::FinderFailed => "發生問題，尋找已停止。你的模組已恢復原狀。",
        M::FinderStopped => "已停止尋找。你的模組已恢復原狀。",
        M::FinderStop => "停止",
        M::FinderInterruptedTitle => "要完成在「{name}」中尋找模組嗎？",
        M::FinderInterrupted => "啟動器在尋找途中關閉了，所以「{name}」的部分模組仍處於停用狀態。",
        M::FinderPutBackFailed => "部分模組無法重新啟用：{error}",

        M::AddingName => "正在加入 {name}",
        M::ModrinthUnreachable => "無法連線到 Modrinth。",
        M::ModpackAddedAlready => {
            "此實例已加入 {pack}。若要加入其他版本，請先在「設定」中將其移除。"
        }
        M::CouldntAddModpack => "無法加入模組包",
        M::ModpackAdded => "模組包已加入",
        M::ModpackAddedMessage => "{pack} 已加入 {name}。按「開始遊戲」即可啟動。",
        M::CouldntRemoveModpack => "無法移除模組包",
        M::ModpackRemoved => "模組包已移除",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::CheckSummary => {
            ["已在 {ms} 毫秒內檢查 {count} 個遊戲檔案：{unchanged} 個自上次檢查後未變更，{hashed} 個已計算雜湊，{downloaded} 個已下載。";
                2]
        }
        P::FilesRepaired => ["已重新下載 {count} 個遺失或損毀的檔案。{summary}"; 2],
        P::BrokenFolders => ["有 {count} 個實例資料夾無法讀取，例如 {path}。"; 2],
        P::FilesSaved => ["已將 {count} 個檔案儲存至 {path}。"; 2],
        P::CurseForgeLeftOut => {
            ["有 {count} 個檔案僅在 CurseForge 上，CurseForge 的檔案不能放進 Modrinth 模組包，因此已略過：{names}。";
                2]
        }
        P::FilesLeftOut => ["有 {count} 個檔案被略過，例如 {name}。"; 2],
        P::LogLinesSkipped => ["前 {count} 行未顯示"; 2],
        P::LinesShown => ["顯示 {shown} 行，共 {count} 行"; 2],
        P::LinesShownFollowing => ["顯示 {shown} 行，共 {count} 行 · 自動捲動"; 2],
        P::ProblemAndMore => ["{problem}（另有 {count} 個問題）"; 2],
        P::InstancesToCopy => ["將複製 {count} 個實例，共 {size}。"; 2],
        P::ImportInstances => ["匯入 {count} 個實例"; 2],
        P::ShareCodeInstalled => ["已從分享碼安裝 {count} 項內容。"; 2],
        P::PackFilesToDownload => {
            ["有 {count} 個檔案需要從 CurseForge 網站下載：實例的「內容」分頁會列出它們。"; 2]
        }
        P::PackFilesLeftOut => ["模組包中有 {count} 個檔案被略過，例如 {path}。"; 2],
        P::WorldsBackedUp => ["已先備份 {count} 個世界。"; 2],
        P::DownloadingFiles => ["正在下載 {count} 個檔案"; 2],
        P::FinderModsLeft => ["還有 {count} 個模組待檢查"; 2],
        P::FinderModsOff => ["已停用 {count} 個模組"; 2],
        P::FinderTriesLeft => ["大約還需 {count} 次嘗試"; 2],

        P::AddedPackFilesRemoved => ["已移除 {pack} 的 {count} 個檔案。"; 2],
        P::AddedPackFilesKept => ["你修改過的 {count} 個檔案已保留。"; 2],
    }
}
