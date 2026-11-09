use crate::i18n::{ContentMsg as M, ContentPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KindMods => "模組",
        M::KindResourcePacks => "資源包",
        M::KindShaders => "光影",
        M::Vanilla => "原版",
        M::SearchInstalledOrModrinth => "搜尋已安裝內容或 Modrinth",
        M::InstalledCount => "已安裝 · {count}",
        M::EnabledCount => "已啟用 · {count} · 重疊時以最上面的包為準",
        M::AvailableCount => "可用 · {count}",
        M::OnModrinthCount => "Modrinth 上 · {count}",
        M::OnModrinth => "Modrinth 上",
        M::NoInstalledMatch => "已安裝 · {count} · 沒有符合的已安裝{kind}。",
        M::OnlyTarget => "僅限 {target}",

        M::StopGameFirst => "請先停止遊戲：遊戲結束時會覆寫這些檔案。",
        M::WaitForInstall => "請等候安裝完成。",
        M::GameRunning => {
            "遊戲正在執行。停止遊戲後才能變更其內容：遊戲結束時會覆寫 options.txt 和光影設定。"
        }

        M::AllModsUpToDate => "來自 Modrinth 的模組皆已是最新版本。",
        M::AllResourcePacksUpToDate => "來自 Modrinth 的資源包皆已是最新版本。",
        M::AllShaderPacksUpToDate => "來自 Modrinth 的光影包皆已是最新版本。",
        M::CouldntCheckUpdates => "無法檢查更新：{error}",

        M::CheckingProject => "正在檢查 {name}",
        M::CheckingDependencies => "正在檢查相依項目",
        M::FilesProgress => "{done}/{total} 個檔案",
        M::BytesProgress => "{done} / {total}",

        M::NothingInstalled => "沒有安裝任何內容。",
        M::InstalledOne => "已安裝 {name}。",
        M::InstalledMany => "已安裝 {names} 和 {last}。",
        M::Replaced => "已取代 {files}。",
        M::Skipped => "已略過 {name}：{reason}。",

        M::AddedFiles => "已加入 {files}。",
        M::FileError => "{name}：{error}",
        M::NotAddedFiles => "未加入：{files}",
        M::AddedAndNotAdded => "已加入 {added}。未加入：{failed}",
        M::AddToMods => "加入模組",
        M::AddToResourcePacks => "加入資源包",
        M::AddToShaders => "加入光影",

        M::LinkNotInstallable => {
            "此連結指向「{kind}」類型的專案，Lodestone Launcher 目前還不支援安裝。"
        }

        M::CheckForUpdates => "檢查更新",
        M::UpdateAll => "全部更新（{count}）",
        M::AddFile => "加入檔案…",
        M::ItemFolder => "{name} · 資料夾",
        M::Compatible => "相容",
        M::OlderVersion => "較舊版本",
        M::NewerVersion => "較新版本",
        M::NotOnModrinth => "不在 Modrinth 上",
        M::UpdateTo => "更新至 {version}",
        M::NoModsYet => "還沒有模組。瀏覽 Modrinth，或將 .jar 檔案拖放到這裡。",
        M::NoResourcePacksYet => "還沒有資源包。瀏覽 Modrinth，或將 .zip 檔案或資料夾拖放到這裡。",
        M::NoShaderPacksYet => "還沒有光影包。瀏覽 Modrinth，或將 .zip 檔案或資料夾拖放到這裡。",
        M::ReadingFolder => "正在讀取 {folder} 資料夾",
        M::CouldntReadFolder => "無法讀取 {folder} 資料夾：{error}",
        M::SomeFilesNotMatched => "部分檔案無法與 Modrinth 比對：{error}",
        M::ShadersOff => "關閉光影",
        M::NormalLook => "Minecraft 的原始畫面。",

        M::ModsNeedLoader => "模組需要模組載入器",
        M::ShadersNeedLoader => "光影需要模組載入器",
        M::VanillaSwitchMods => {
            "此實例是原版 Minecraft。模組需要透過 {loader} 等模組載入器執行；切換會保留你的世界和設定，{loader} 會在你下次按下「開始遊戲」時安裝。"
        }
        M::VanillaSwitchShaders => {
            "此實例是原版 Minecraft。光影需要透過 {loader} 等模組載入器執行；切換會保留你的世界和設定，{loader} 會在你下次按下「開始遊戲」時安裝。"
        }
        M::VanillaNoLoaderMods => {
            "此實例是原版 Minecraft {version}。模組需要模組載入器，但 Lodestone Launcher 支援的載入器都無法在此版本上執行：Fabric 從 1.14 開始支援，Forge 也沒有適用於此版本的建置。使用較新版本的新實例即可使用模組。"
        }
        M::VanillaNoLoaderShaders => {
            "此實例是原版 Minecraft {version}。光影需要模組載入器，但 Lodestone Launcher 支援的載入器都無法在此版本上執行：Fabric 從 1.14 開始支援，Forge 也沒有適用於此版本的建置。使用較新版本的新實例即可使用光影。"
        }
        M::VanillaCheckFailedMods => {
            "此實例是原版 Minecraft。模組需要模組載入器，但無法檢查哪個載入器適用：{error}"
        }
        M::VanillaCheckFailedShaders => {
            "此實例是原版 Minecraft。光影需要模組載入器，但無法檢查哪個載入器適用：{error}"
        }
        M::VanillaCheckingMods => {
            "此實例是原版 Minecraft。模組需要透過模組載入器執行；正在檢查哪個載入器能在 Minecraft {version} 上執行…"
        }
        M::VanillaCheckingShaders => {
            "此實例是原版 Minecraft。光影需要透過模組載入器執行；正在檢查哪個載入器能在 Minecraft {version} 上執行…"
        }
        M::SwitchTo => "切換至 {loader}",
        M::NewFabricInstance => "新增 Fabric 實例…",

        M::ShaderModUsingPack => "{shader_mod} 正在使用 {pack}。",
        M::ShaderModUsingBuiltIn => "{shader_mod} 正在使用內建光影。",
        M::ShaderModInstalledOff => "{shader_mod} 已安裝；光影已關閉。",
        M::ShaderModPickPack => {
            "請在下方選擇一個光影包。{shader_mod} 會在遊戲啟動時從 {file} 讀取你的選擇。"
        }
        M::ShadersNeedShaderMod => "光影需要光影模組",
        M::OptiFineNote => {
            "OptiFine 也能使用，但它不在 Modrinth 上，且其授權條款不允許 Lodestone 下載。如果你偏好使用它，請從 optifine.net 下載，然後在「{mods}」中用「{add_file}」加入 jar 檔（Fabric 還需要 OptiFabric）。"
        }
        M::LookingForShaderMod => "正在 Modrinth 上尋找適用於此版本的光影模組",
        M::CouldntAskModrinth => "無法連線至 Modrinth：{error}",
        M::InstallShaderMod => "安裝 {shader_mod} {version}",
        M::ShaderModRunsOn => {
            "Minecraft 本身無法載入光影包，需要借助光影模組。{shader_mod} 可在此實例的 {loader} 上執行。"
        }
        M::ShaderModComesWith => "它會一併安裝 {shader_mod} 所需的 {dependency}。",
        M::ShaderModUnavailable => {
            "Minecraft 本身無法載入光影包，需要借助光影模組。{reason} Fabric 實例一律可以使用 Iris。"
        }

        M::BrowsingNeedsNetwork => "瀏覽 Modrinth 需要網路連線。",
        M::SortRelevance => "關聯性",
        M::SortDownloads => "下載次數",
        M::SortFollows => "追蹤數",
        M::SortNewest => "最新",
        M::SortUpdated => "最近更新",
        M::SearchingModrinth => "正在搜尋 Modrinth",
        M::NothingMatches => "Modrinth 上沒有符合的結果。",
        M::PageOf => "第 {page} 頁，共 {pages} 頁",
        M::ByAuthor => "作者：{author}",
        M::InstalledBadge => "已安裝",
        M::OptionalOnServers => "伺服器上可選用",
        M::DownloadsCompact => "{count}次下載",
        M::FollowsCompact => "{count}人追蹤",
        M::FollowersCompact => "{count}位追蹤者",

        M::LoadingProject => "正在載入專案",
        M::ViewOnModrinth => "在 Modrinth 上檢視",
        M::Versions => "版本",
        M::Description => "介紹",
        M::LoadingVersions => "正在載入版本",
        M::ShowAllVersions => "顯示適用於其他載入器和 Minecraft 版本的版本",
        M::AlsoInstallDependencies => "一併安裝必要的相依項目",
        M::NoVersionFits => {
            "{project} 沒有適用於此實例的版本。開啟「{show_all}」即可查看所有版本。"
        }
        M::ShowVersionsShort => "顯示版本…",
        M::InstallAnyway => "仍要安裝…",
        M::InstallItAnyway => "確定仍要安裝",
        M::InstallWith => "與 {shader_mod} 一併安裝",
        M::Release => "正式版",
        M::Beta => "測試版",
        M::Alpha => "早期測試版",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Downloads => ["{count} 次下載"; 2],
        P::Follows => ["{count} 人追蹤"; 2],
        P::Followers => ["{count} 位追蹤者"; 2],
        P::MoreVersions => ["Modrinth 上還有 {count} 個版本。"; 2],
    }
}
