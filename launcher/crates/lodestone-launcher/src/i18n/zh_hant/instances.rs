use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::TwoSentences => "{first}{second}",
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
        M::ProgressDetail => "{files_done}/{files_total} · {done} MB/{total} MB",

        M::PreparingNote => "正在準備 Minecraft {version}（{loader}）",
        M::JoiningNote => "遊戲啟動後將加入 {address}",
        M::VerifyingNote => "正在驗證 Minecraft {version}（{loader}）的遊戲檔案",
        M::RosettaNote => "此版本透過 Rosetta 2 在 Intel（x86_64）Java 中執行。",
        M::StartingJavaNote => "正在啟動 Java：{command}",
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
        M::CouldntStartMinecraft => "無法啟動 Minecraft",
        M::CouldntStartJava => "無法啟動 Java",
        M::CouldntVerify => "無法驗證遊戲檔案",
        M::GameFilesVerified => "遊戲檔案已驗證",
        M::EveryFileIntact => "所有檔案皆完好無缺。{summary}",
        M::ModpackUpdating => "模組包正在更新",
        M::WaitForUpdate => "請等待更新完成，再按「開始遊戲」。",
        M::StopBeforeDeleting => "請先停止遊戲，再刪除該實例。",
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
        M::StopAndClose => "停止並關閉",

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
        P::LinesShown => ["顯示 {shown} 行，共 {count} 行"; 2],
        P::LinesShownFollowing => ["顯示 {shown} 行，共 {count} 行 · 自動捲動"; 2],
        P::ProblemAndMore => ["{problem}（另有 {count} 個問題）"; 2],
        P::InstancesToCopy => ["將複製 {count} 個實例，共 {size}。"; 2],
        P::ImportInstances => ["匯入 {count} 個實例"; 2],
        P::PackFilesToDownload => {
            ["有 {count} 個檔案需要從 CurseForge 網站下載：實例的「內容」分頁會列出它們。"; 2]
        }
        P::PackFilesLeftOut => ["模組包中有 {count} 個檔案被略過，例如 {path}。"; 2],
        P::WorldsBackedUp => ["已先備份 {count} 個世界。"; 2],
        P::DownloadingFiles => ["正在下載 {count} 個檔案"; 2],
    }
}
