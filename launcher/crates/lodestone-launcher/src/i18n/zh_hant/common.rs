use crate::i18n::{CommonMsg as M, CommonPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Add => "新增",
        M::Apply => "套用",
        M::Back => "返回",
        M::Browse => "瀏覽…",
        M::Cancel => "取消",
        M::Change => "變更…",
        M::Close => "關閉",
        M::Continue => "繼續",
        M::Copy => "複製",
        M::Copied => "已複製",
        M::Default => "預設",
        M::Delete => "刪除",
        M::Discard => "捨棄",
        M::Done => "完成",
        M::Edit => "編輯",
        M::Install => "安裝",
        M::Loading => "正在載入…",
        M::Next => "下一頁",
        M::No => "否",
        M::None => "無",
        M::NotNow => "稍後再說",
        M::Off => "關閉",
        M::Ok => "確定",
        M::On => "開啟",
        M::Open => "開啟",
        M::OpenFolder => "開啟資料夾",
        M::Previous => "上一頁",
        M::Refresh => "重新整理",
        M::Remove => "移除",
        M::Rename => "重新命名",
        M::Reset => "重置",
        M::Save => "儲存",
        M::Saved => "已儲存",
        M::Search => "搜尋",
        M::SignIn => "登入",
        M::SignOut => "登出",
        M::Start => "啟動",
        M::Stop => "停止",
        M::TryAgain => "再試一次",
        M::Update => "更新",
        M::Yes => "是",
        M::KeepEditing => "繼續編輯",
        M::UnsavedChanges => "你有未儲存的變更",
        M::ShowInFileManager => {
            if cfg!(target_os = "macos") {
                "在 Finder 中顯示"
            } else if cfg!(windows) {
                "在檔案總管中顯示"
            } else {
                "在資料夾中顯示"
            }
        }
        M::MoveToTrash => {
            if cfg!(windows) {
                "移至資源回收筒"
            } else {
                "移到垃圾桶"
            }
        }
        M::TheTrash => {
            if cfg!(windows) {
                "資源回收筒"
            } else {
                "垃圾桶"
            }
        }
        M::OpenImage => {
            if cfg!(target_os = "macos") {
                "在「預覽程式」中開啟"
            } else {
                "開啟"
            }
        }
        M::ThisComputer | M::ThisComputerStart => {
            if cfg!(target_os = "macos") {
                "這台 Mac"
            } else {
                "這台電腦"
            }
        }
        M::Keychain => {
            if cfg!(target_os = "macos") {
                "鑰匙圈"
            } else if cfg!(windows) {
                "認證管理員"
            } else {
                "金鑰圈"
            }
        }
        M::KeychainInFull => {
            if cfg!(target_os = "macos") {
                "macOS 鑰匙圈"
            } else if cfg!(windows) {
                "Windows 認證管理員"
            } else {
                "系統金鑰圈"
            }
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Instances => ["{count} 個實例"; 2],
        P::Files => ["{count} 個檔案"; 2],
    }
}
