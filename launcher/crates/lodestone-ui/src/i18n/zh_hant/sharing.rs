use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByOther => "由其他程式製作",
        M::ImageCarried => "在檔案本身中，不是從 Modrinth 下載的",
        M::ImageClientOnly => "僅玩家使用的模組，已排除",
        M::ImageNotDownloadable => {
            "Lodestone 伺服器暫時無法從 CurseForge 下載，因此這些檔案不會被安裝。"
        }
        M::ImageTakenOut => "傳送者已移除",
        M::ImageBlanked => "傳送者已清空",
        M::ImageSecretsNotTaken => "未匯入，因為其中含有伺服器金鑰、密碼或權杖",
        M::ImageWithheld => "檔案中缺少",
        M::ImageNotTaken => "未從檔案匯入",
        M::NotTakenSoftware => "伺服器軟體",
        M::NotTakenEula => "終端使用者授權合約",
        M::NotTakenClientFiles => "玩家自己的檔案",
        M::NotTakenOperators => "管理員",
        M::NotTakenScripts => "腳本",
        M::ImageOfflineMode => "玩家無需 Minecraft 帳號即可加入。",
        M::ImageServerSoftware => "伺服器軟體",
        M::ImageServerSoftwareHint => "Paper 可以執行外掛程式。原版伺服器按原樣執行 Minecraft。",
        M::ImageWorld => "世界",
        M::ImageWorldHint => "只有這個世界會複製到伺服器。",
        M::ImageNoWorld => "不使用世界（新建一個）",
        M::ImageDownloads => "從 Modrinth 下載",
        M::ImageDisk => "磁碟空間",
        M::ImageNoRoom => {
            "可用空間不足，無法用檔案自帶的世界建立此伺服器。較小的世界或不選世界也許可以。"
        }
        M::ImageNeedsGlobal => {
            "此檔案包含自己的 jar 檔案，因此用它建立伺服器需要修改全域檔案的權限。"
        }
        M::ImageProtectedWarning => {
            "它包含製作者自帶的受保護檔案，例如 jar 檔案或程式。請只使用你信任的人提供的檔案。"
        }
        M::ImageMadeAsPaper => "Lodestone 會將這類伺服器建立為 Paper 伺服器，它們執行相同的外掛。",
        M::ImageUnsupported => "Lodestone 無法用此載入器建立伺服器。",
        M::ImageMadeWith => "建立工具",
        M::ImageFiles => "檔案中的檔案數",
        M::ImageSize => "檔案大小",
        M::ImageBasedOn => "基於",
        M::ImageNotInstalled => "不會安裝",
        M::ImageOff => "關閉",
        M::ImageDownloadedMods => "從 Modrinth 下載",
        M::ImageDiskSpare => "所需空間包含 1 GB 餘量。",
        M::NotTakenSoftwareReason => "Lodestone 會安裝自己的",
        M::NotTakenOperatorsReason => "可以保留",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "其中有 {count} 個模組或外掛：在伺服器上執行的程式。只使用你信任的人提供的檔案。",
            "其中有 {count} 個模組或外掛：在伺服器上執行的程式。只使用你信任的人提供的檔案。",
        ],
        P::OutcomeNotDownloaded => [
            "{count} 個檔案無法下載，已排除",
            "{count} 個檔案無法下載，已排除",
        ],
        P::OutcomeNoSource => [
            "{count} 個檔案無法由 Lodestone 伺服器下載，已排除",
            "{count} 個檔案無法由 Lodestone 伺服器下載，已排除",
        ],
        P::OutcomeClientJars => [
            "已移除 {count} 個僅在玩家電腦上執行的模組",
            "已移除 {count} 個僅在玩家電腦上執行的模組",
        ],
        P::OutcomeSecrets => [
            "{count} 個含有伺服器金鑰、密碼或權杖的檔案已排除，伺服器將寫入預設值",
            "{count} 個含有伺服器金鑰、密碼或權杖的檔案已排除，伺服器將寫入預設值",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} 個設定檔無法檢查是否含有伺服器金鑰，已排除，伺服器將寫入預設值",
            "{count} 個設定檔無法檢查是否含有伺服器金鑰，已排除，伺服器將寫入預設值",
        ],
        P::OutcomeScripts => [
            "{count} 個指令碼已略過：只有擁有者能在啟用程式碼執行時新增指令碼。",
            "{count} 個指令碼已略過：只有擁有者能在啟用程式碼執行時新增指令碼。",
        ],
        P::ImageMoreNames => ["另有 {count} 個未列出", "另有 {count} 個未列出"],
    }
}
