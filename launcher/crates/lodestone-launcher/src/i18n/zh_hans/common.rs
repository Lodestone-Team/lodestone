use crate::i18n::{CommonMsg as M, CommonPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Add => "添加",
        M::Apply => "应用",
        M::Back => "返回",
        M::Browse => "浏览…",
        M::Cancel => "取消",
        M::Change => "更改…",
        M::Close => "关闭",
        M::Continue => "继续",
        M::Copy => "复制",
        M::Copied => "已复制",
        M::Default => "默认",
        M::Delete => "删除",
        M::Discard => "放弃",
        M::Done => "完成",
        M::Edit => "编辑",
        M::Install => "安装",
        M::Loading => "正在加载…",
        M::Next => "下一页",
        M::No => "否",
        M::None => "无",
        M::NotNow => "以后再说",
        M::Off => "关",
        M::Ok => "确定",
        M::On => "开",
        M::Open => "打开",
        M::OpenFolder => "打开文件夹",
        M::Previous => "上一页",
        M::Refresh => "刷新",
        M::Remove => "移除",
        M::Rename => "重命名",
        M::Reset => "重置",
        M::Save => "保存",
        M::Saved => "已保存",
        M::Search => "搜索",
        M::SignIn => "登录",
        M::SignOut => "退出登录",
        M::Start => "启动",
        M::Stop => "停止",
        M::TryAgain => "重试",
        M::Update => "更新",
        M::Yes => "是",
        M::KeepEditing => "继续编辑",
        M::UnsavedChanges => "你有未保存的更改",
        M::ShowInFileManager => {
            if cfg!(target_os = "macos") {
                "在访达中显示"
            } else if cfg!(windows) {
                "在文件资源管理器中显示"
            } else {
                "在文件夹中显示"
            }
        }
        M::MoveToTrash => {
            if cfg!(target_os = "macos") {
                "移到废纸篓"
            } else {
                "移到回收站"
            }
        }
        M::TheTrash => {
            if cfg!(target_os = "macos") {
                "废纸篓"
            } else {
                "回收站"
            }
        }
        M::OpenImage => {
            if cfg!(target_os = "macos") {
                "在“预览”中打开"
            } else {
                "打开"
            }
        }
        M::ThisComputer | M::ThisComputerStart => {
            if cfg!(target_os = "macos") {
                "这台 Mac"
            } else {
                "这台电脑"
            }
        }
        M::Keychain => {
            if cfg!(target_os = "macos") {
                "钥匙串"
            } else if cfg!(windows) {
                "凭据管理器"
            } else {
                "密钥环"
            }
        }
        M::KeychainInFull => {
            if cfg!(target_os = "macos") {
                "macOS 钥匙串"
            } else if cfg!(windows) {
                "Windows 凭据管理器"
            } else {
                "系统密钥环"
            }
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Instances => ["{count} 个实例"; 2],
        P::Files => ["{count} 个文件"; 2],
    }
}
