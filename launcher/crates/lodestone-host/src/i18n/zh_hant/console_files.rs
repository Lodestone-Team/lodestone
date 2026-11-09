use crate::i18n::{ConsoleFilesMsg as M, ConsoleFilesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ConsoleSearch => "搜尋主控台…",
        M::ConsoleRegex => "規則運算式",
        M::ConsoleStatusNoPermission => "沒有存取主控台的權限",
        M::ConsoleStatusLoading => "主控台載入中…",
        M::ConsoleStatusBuffered => "歷史訊息，沒有即時更新",
        M::ConsoleStatusLive => "主控台已即時連線",
        M::ConsoleStatusLiveNoBuffer => {
            "主控台已即時連線，但無法取得歷史記錄。你的網路連線可能不穩定"
        }
        M::ConsoleStatusClosed => "主控台已關閉",
        M::ConsoleStatusError => "連線中斷或發生錯誤",
        M::ConsoleServerStopped => "伺服器已停止",
        M::ConsoleInputNoPermission => "沒有權限",
        M::ConsoleInputPlaceholder => "輸入指令…",
        M::ConsoleNoPermission => "你沒有存取此主控台的權限",
        M::ConsoleLoading => "主控台載入中",
        M::ConsoleEmpty => "主控台還沒有訊息",
        M::ConsoleSendFailed => "無法傳送指令：{error}",
        M::ConsoleLinkedUser => "連結了此 Minecraft 帳戶的 Lodestone 使用者",
        M::ConsoleOwnerOnlyAtom => "只有擁有者可以控制 atom 伺服器",
        M::ConsoleJumpToLatest => "跳至最新",
        M::Console => "主控台",
        M::ConsoleCommand => "指令",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
