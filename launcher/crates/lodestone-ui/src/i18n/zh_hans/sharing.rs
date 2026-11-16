use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByHost => "由 Lodestone Host {version} 制作",
        M::ImageMadeByLauncher => "由 Lodestone Launcher {version} 制作",
        M::ImageMadeByOther => "由其他程序制作",
        M::ImageCarried => "这些内容包含在文件本身中，而不是从 Modrinth 下载：{names}",
        M::ImageClientOnly => "仅玩家使用的模组，已略过：{names}",
        M::ImageNotDownloadable => {
            "Lodestone 服务器暂时无法从 CurseForge 下载，因此不会安装这些文件：{names}"
        }
        M::ImageTakenOut => "发送者移除的内容：{names}",
        M::ImageBlanked => "发送者清空的内容：{names}",
        M::ImageSecretsNotTaken => "未获取，因为其中含有服务器密钥、密码或令牌：{names}",
        M::ImageWithheld => "文件中缺少：{names}",
        M::ImageNotTaken => "不会从文件中获取：{what}",
        M::NotTakenSoftware => "服务器软件（Lodestone 会安装自己的）",
        M::NotTakenEula => "EULA",
        M::NotTakenClientFiles => "玩家自己的文件",
        M::NotTakenOperators => "管理员",
        M::NotTakenScripts => "脚本",
        M::ImageOfflineMode => "其设置允许玩家无需 Minecraft 账户即可加入（online-mode 已关闭）。",
        M::ImageServerSoftware => "服务器软件",
        M::ImageServerSoftwareHint => "Paper 可以运行插件。原版服务器按原样运行 Minecraft。",
        M::ImageWorld => "世界",
        M::ImageWorldHint => "只有这个世界会复制到服务器。",
        M::ImageNoWorld => "不使用世界（新建一个）",
        M::ImageDownloads => "从 Modrinth 下载 {size}",
        M::ImageDownloadsMods => "从 Modrinth 下载 {size}：{names}",
        M::ImageDisk => "需要 {needed} 的磁盘空间，其中 1 GB 为预留；可用 {free}。",
        M::ImageNoRoom => {
            "可用空间不足，无法用文件自带的世界创建此服务器。较小的世界或不选世界也许可以。"
        }
        M::ImageNeedsGlobal => {
            "此文件包含自己的 jar 文件，因此用它创建服务器需要修改全局文件的权限。"
        }
        M::ImageProtectedWarning => {
            "它包含制作者自带的受保护文件，例如 jar 文件或程序。请只使用你信任的人提供的文件。"
        }
        M::ImageMadeAsPaper => {
            "Lodestone 会把 {flavour} 服务器做成 Paper 服务器，可运行相同的插件。"
        }
        M::ImageUnsupported => "Lodestone 无法创建 {loader} 服务器。",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "其中有 {count} 个模组或插件：在服务器上运行的程序。只使用你信任的人提供的文件。",
            "其中有 {count} 个模组或插件：在服务器上运行的程序。只使用你信任的人提供的文件。",
        ],
        P::OutcomeNotDownloaded => [
            "{count} 个文件无法下载，已略过：{names}",
            "{count} 个文件无法下载，已略过：{names}",
        ],
        P::OutcomeNoSource => [
            "{count} 个文件无法由 Lodestone 服务器下载，已略过：{names}",
            "{count} 个文件无法由 Lodestone 服务器下载，已略过：{names}",
        ],
        P::OutcomeClientJars => [
            "{count} 个只在玩家电脑上运行的模组已移除：{names}",
            "{count} 个只在玩家电脑上运行的模组已移除：{names}",
        ],
        P::OutcomeSecrets => [
            "{count} 个含有服务器密钥、密码或令牌的文件已略过，服务器会写入默认设置：{names}",
            "{count} 个含有服务器密钥、密码或令牌的文件已略过，服务器会写入默认设置：{names}",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} 个设置文件无法检查是否含有服务器密钥，已略过，服务器会写入默认设置：{names}",
            "{count} 个设置文件无法检查是否含有服务器密钥，已略过，服务器会写入默认设置：{names}",
        ],
        P::OutcomeScripts => [
            "{count} 个脚本已略过：只有所有者能在启用代码执行时添加脚本。",
            "{count} 个脚本已略过：只有所有者能在启用代码执行时添加脚本。",
        ],
    }
}
