use crate::i18n::{ValueMsg as M, ValuePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KeyValue => "{key}：{value}",
        M::FromTo => "从 {from} 变为 {to}",
        M::NeededOfFree => "需要 {needed}，可用 {free}",
        M::Online => "在线",
        M::Offline => "离线",
        M::Running => "运行中",
        M::Stopped => "已停止",
        M::Starting => "正在启动",
        M::Stopping => "正在停止",
        M::Failed => "失败",
        M::Installing => "正在安装",
        M::Downloading => "正在下载",
        M::Copying => "正在复制",
        M::Paused => "已暂停",
        M::KindFolder => "文件夹",
        M::KindWorld => "世界",
        M::KindMod => "模组",
        M::KindResourcePack => "资源包",
        M::KindShaderPack => "光影包",
        M::KindDataPack => "数据包",
        M::KindPlugin => "插件",
        M::KindModpack => "整合包",
        M::KindImage => "图片",
        M::KindServer => "服务器",
        M::KindInstance => "实例",
        M::KindArchive => "压缩包",
        M::KindFile => "文件",
        M::KindBackup => "备份",
        M::KindSnapshot => "快照",
        M::KindFriend => "好友",
        M::KindComputer => "电脑",
        M::UntitledSnapshot => "未命名快照",
        M::UntitledBackup => "未命名备份",
        M::BeforeUpdate => "更新前",
        M::BeforeRestore => "还原前",
        M::CopyOf => "复制自",
        M::Automatic => "自动",
        M::Added => "已添加",
        M::Skipped => "已跳过",
        M::NotAdded => "未添加",
        M::Reveal => "显示",
        M::Hide => "隐藏",
        M::Hidden => "已隐藏",
        M::OpenLink => "打开链接",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShowMore => ["再显示 {count} 项", "再显示 {count} 项"],
    }
}
