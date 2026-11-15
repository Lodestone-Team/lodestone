use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByHost => "由 Lodestone Host {version} 製作",
        M::ImageMadeByLauncher => "由 Lodestone Launcher {version} 製作",
        M::ImageMadeByOther => "由其他程式製作",
        M::ImageCarried => "這些內容包含在檔案本身中，而不是從 Modrinth 下載：{names}",
        M::ImageClientOnly => "僅玩家使用的模組，已略過：{names}",
        M::ImageNotDownloadable => {
            "Lodestone 伺服器暫時無法從 CurseForge 下載，因此不會安裝這些檔案：{names}"
        }
        M::ImageTakenOut => "傳送者移除的內容：{names}",
        M::ImageBlanked => "傳送者清空的內容：{names}",
        M::ImageSecretsNotTaken => "未取得，因為其中含有伺服器金鑰、密碼或權杖：{names}",
        M::ImageWithheld => "檔案中缺少：{names}",
        M::ImageNotTaken => "不會從檔案中取得：{what}",
        M::NotTakenSoftware => "伺服器軟體（Lodestone 會安裝自己的）",
        M::NotTakenEula => "EULA",
        M::NotTakenClientFiles => "玩家自己的檔案",
        M::NotTakenOperators => "管理員",
        M::NotTakenScripts => "腳本",
        M::ImageOfflineMode => "其設定允許玩家無需 Minecraft 帳號即可加入（online-mode 已關閉）。",
        M::ImageServerSoftware => "伺服器軟體",
        M::ImageServerSoftwareHint => "Paper 可以執行外掛程式。原版伺服器按原樣執行 Minecraft。",
        M::ImageWorld => "世界",
        M::ImageWorldHint => "成為伺服器世界的那個世界。",
        M::ImageNoWorld => "不使用世界（新建一個）",
        M::ImageDownloads => "從 Modrinth 下載 {size}",
        M::ImageDownloadsMods => "從 Modrinth 下載 {size}：{names}",
        M::ImageDisk => "需要 {needed} 的磁碟空間，其中 1 GB 為預留；可用 {free}。",
        M::ImageNoRoom => {
            "可用空間不足，無法用檔案自帶的世界建立此伺服器。較小的世界或不選世界也許可以。"
        }
        M::ImageNeedsGlobal => {
            "此檔案包含自己的 jar 檔案，因此用它建立伺服器需要修改全域檔案的權限。"
        }
        M::ImageProtectedWarning => {
            "它包含製作者自帶的受保護檔案，例如 jar 檔案或程式。請只使用你信任的人提供的檔案。"
        }
        M::ImageMadeAsPaper => {
            "Lodestone 會把 {flavour} 伺服器做成 Paper 伺服器，可執行相同的外掛。"
        }
        M::ImageUnsupported => "Lodestone 無法建立 {loader} 伺服器。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "其中有 {count} 個模組或外掛：在伺服器上執行的程式。只使用你信任的人提供的檔案。",
            "其中有 {count} 個模組或外掛：在伺服器上執行的程式。只使用你信任的人提供的檔案。",
        ],
        P::OutcomeNotDownloaded => [
            "{count} 個檔案無法下載，已略過：{names}",
            "{count} 個檔案無法下載，已略過：{names}",
        ],
        P::OutcomeNoSource => [
            "{count} 個檔案無法由 Lodestone 伺服器下載，已略過：{names}",
            "{count} 個檔案無法由 Lodestone 伺服器下載，已略過：{names}",
        ],
        P::OutcomeClientJars => [
            "{count} 個只在玩家電腦上執行的模組已移除：{names}",
            "{count} 個只在玩家電腦上執行的模組已移除：{names}",
        ],
        P::OutcomeSecrets => [
            "{count} 個含有伺服器金鑰、密碼或權杖的檔案已略過，伺服器會寫入預設設定：{names}",
            "{count} 個含有伺服器金鑰、密碼或權杖的檔案已略過，伺服器會寫入預設設定：{names}",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} 個設定檔無法檢查是否含有伺服器金鑰，已略過，伺服器會寫入預設設定：{names}",
            "{count} 個設定檔無法檢查是否含有伺服器金鑰，已略過，伺服器會寫入預設設定：{names}",
        ],
        P::OutcomeScripts => [
            "{count} 個指令碼已略過：只有擁有者能在啟用程式碼執行時新增指令碼。",
            "{count} 個指令碼已略過：只有擁有者能在啟用程式碼執行時新增指令碼。",
        ],
    }
}
