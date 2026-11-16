use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByOther => "由其他程序制作",
        M::ImageCarried => "在文件本身中，不是从 Modrinth 下载的",
        M::ImageClientOnly => "仅玩家使用的模组，已排除",
        M::ImageNotDownloadable => {
            "Lodestone 服务器暂时无法从 CurseForge 下载，因此这些文件不会被安装。"
        }
        M::ImageTakenOut => "发送者已移除",
        M::ImageBlanked => "发送者已清空",
        M::ImageSecretsNotTaken => "未导入，因为其中含有服务器密钥、密码或令牌",
        M::ImageWithheld => "文件中缺少",
        M::ImageNotTaken => "未从文件导入",
        M::NotTakenSoftware => "服务器软件",
        M::NotTakenEula => "最终用户许可协议",
        M::NotTakenClientFiles => "玩家自己的文件",
        M::NotTakenOperators => "管理员",
        M::NotTakenScripts => "脚本",
        M::ImageOfflineMode => "玩家无需 Minecraft 账户即可加入。",
        M::ImageServerSoftware => "服务器软件",
        M::ImageServerSoftwareHint => "Paper 可以运行插件。原版服务器按原样运行 Minecraft。",
        M::ImageWorld => "世界",
        M::ImageWorldHint => "成为服务器世界的那个世界。",
        M::ImageNoWorld => "不使用世界（新建一个）",
        M::ImageDownloads => "从 Modrinth 下载",
        M::ImageDisk => "磁盘空间",
        M::ImageNoRoom => {
            "可用空间不足，无法用文件自带的世界创建此服务器。较小的世界或不选世界也许可以。"
        }
        M::ImageNeedsGlobal => {
            "此文件包含自己的 jar 文件，因此用它创建服务器需要修改全局文件的权限。"
        }
        M::ImageProtectedWarning => {
            "它包含制作者自带的受保护文件，例如 jar 文件或程序。请只使用你信任的人提供的文件。"
        }
        M::ImageMadeAsPaper => "Lodestone 会将这类服务器创建为 Paper 服务器，它们运行相同的插件。",
        M::ImageUnsupported => "Lodestone 无法用此加载器创建服务器。",
        M::ImageMadeWith => "创建工具",
        M::ImageFiles => "文件中的文件数",
        M::ImageSize => "文件大小",
        M::ImageBasedOn => "基于",
        M::ImageNotInstalled => "不会安装",
        M::ImageOff => "关闭",
        M::ImageDownloadedMods => "从 Modrinth 下载",
        M::ImageDiskSpare => "所需空间包含 1 GB 余量。",
        M::NotTakenSoftwareReason => "Lodestone 会安装自己的",
        M::NotTakenOperatorsReason => "可以保留",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "其中有 {count} 个模组或插件：在服务器上运行的程序。只使用你信任的人提供的文件。",
            "其中有 {count} 个模组或插件：在服务器上运行的程序。只使用你信任的人提供的文件。",
        ],
        P::OutcomeNotDownloaded => [
            "{count} 个文件无法下载，已排除",
            "{count} 个文件无法下载，已排除",
        ],
        P::OutcomeNoSource => [
            "{count} 个文件无法由 Lodestone 服务器下载，已排除",
            "{count} 个文件无法由 Lodestone 服务器下载，已排除",
        ],
        P::OutcomeClientJars => [
            "已移除 {count} 个仅在玩家电脑上运行的模组",
            "已移除 {count} 个仅在玩家电脑上运行的模组",
        ],
        P::OutcomeSecrets => [
            "{count} 个含有服务器密钥、密码或令牌的文件已排除，服务器将写入默认值",
            "{count} 个含有服务器密钥、密码或令牌的文件已排除，服务器将写入默认值",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} 个设置文件无法检查是否含有服务器密钥，已排除，服务器将写入默认值",
            "{count} 个设置文件无法检查是否含有服务器密钥，已排除，服务器将写入默认值",
        ],
        P::OutcomeScripts => [
            "{count} 个脚本已略过：只有所有者能在启用代码执行时添加脚本。",
            "{count} 个脚本已略过：只有所有者能在启用代码执行时添加脚本。",
        ],
        P::ImageMoreNames => ["另有 {count} 个未列出", "另有 {count} 个未列出"],
    }
}
