use crate::i18n::{ConsoleFilesMsg as M, ConsoleFilesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ConsoleSearch => "搜索控制台…",
        M::ConsoleRegex => "正则表达式",
        M::ConsoleStatusNoPermission => "没有访问控制台的权限",
        M::ConsoleStatusLoading => "正在加载控制台…",
        M::ConsoleStatusBuffered => "历史消息，没有实时更新",
        M::ConsoleStatusLive => "控制台已实时连接",
        M::ConsoleStatusLiveNoBuffer => {
            "控制台已实时连接，但无法获取历史记录。你的网络连接可能不稳定"
        }
        M::ConsoleStatusClosed => "控制台已关闭",
        M::ConsoleStatusError => "连接已断开或出错",
        M::ConsoleServerStopped => "服务器已停止",
        M::ConsoleInputNoPermission => "没有权限",
        M::ConsoleInputPlaceholder => "输入命令…",
        M::ConsoleNoPermission => "你没有访问此控制台的权限",
        M::ConsoleLoading => "正在加载控制台",
        M::ConsoleEmpty => "控制台还没有消息",
        M::ConsoleSendFailed => "无法发送命令：{error}",
        M::ConsoleLinkedUser => "关联了此 Minecraft 账户的 Lodestone 用户",
        M::ConsoleOwnerOnlyAtom => "只有所有者可以控制 atom 服务器",
        M::ConsoleJumpToLatest => "跳到最新",
        M::Console => "控制台",
        M::ConsoleCommand => "命令",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
