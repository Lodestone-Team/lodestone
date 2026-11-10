use crate::i18n::{FriendListMsg as M, FriendListPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Title => "好友",
        M::Hint => "只需新增一次的好友。不需連結即可查看並加入他們與你分享的伺服器。",
        M::AddFriend => "新增好友",
        M::Refreshing => "正在查看好友狀態…",
        M::You => "你",
        M::YourSafetyCode => "你的安全碼：{code}",
        M::NoNameYet => "先選擇好友會看到的名稱，再用好友連結新增他們。",
        M::ChooseName => "選擇名稱",
        M::EditName => "修改名稱",
        M::OpenFailed => "無法開啟你的好友清單",
        M::ActionFailed => "無法更新你的好友清單",
        M::NoFriendsYet => "還沒有好友。建立一個好友連結，傳給你認識的人。",
        M::RefreshProblem => "有些好友無法查看：{error}",
        M::LinksTitle => "你的好友連結",
        M::LinkWaiting => "正在等待回覆。僅能使用一次，有效期限至 {time}。",
        M::CancelLink => "取消連結",
        M::CheckAnswers => "查看回覆",
        M::ReplyFrom => "{name} 回覆了你的好友連結",
        M::SafetyCode => "安全碼：{code}",
        M::SameNameAs => "你已經有一位叫 {names} 的好友。繼續之前，請確認這是另一個人。",
        M::Accept => "接受",
        M::Ignore => "忽略",
        M::WaitingFor => "正在等待 {name} 確認",
        M::NotVerified => "未驗證",
        M::Verified => "安全碼相符",
        M::Hidden => "已對其隱藏",
        M::Retired => "已停用此好友清單",
        M::TheirName => "對方的名稱：{name}",
        M::OnlineOn => "線上：{cores}",
        M::NotSharingNow => "目前沒有與你分享",
        M::SharesNothing => "尚未與你分享任何伺服器",
        M::Servers => "伺服器",
        M::CompareCodes => "核對安全碼",
        M::HideFromThem => "對其隱藏",
        M::StopHiding => "取消隱藏",
        M::CoreOnline => "線上",
        M::CoreOffline => "目前未分享",
        M::LoadingServers => "正在取得伺服器…",
        M::NoServersShared => "這台電腦上沒有與你分享的內容。",
        M::ServersFailed => "無法列出伺服器：{error}",
        M::ShowServers => "顯示伺服器",
        M::AddTitle => "新增好友",
        M::SendLinkTitle => "傳送好友連結給對方",
        M::SendLinkBody => {
            "建立一個連結並傳給你的好友。對方開啟後，回覆會顯示在伺服器頁面上，等你接受。"
        }
        M::MakeLink => "建立連結",
        M::LinkWorksOnce => {
            "僅能使用一次，24 小時內有效。任何拿到這個連結的人都能請求成為你的好友，所以只傳給對方本人。"
        }
        M::OpenLinkTitle => "或者開啟對方的好友連結",
        M::LinkPlaceholder => "好友連結（https://connect.lodestone.cc/f/…）",
        M::LookUp => "查看",
        M::NotAFriendLink => "這不是好友連結。好友連結以 https://connect.lodestone.cc/f/ 開頭。",
        M::PasteFirst => "請先貼上好友連結。",
        M::OtherService => {
            "這個連結屬於 {host}，不是此啟動器使用的 Lodestone Connect 服務，因此無法在這裡開啟。"
        }
        M::PreviewFrom => "這個連結來自 {name}",
        M::PreviewCompare => "成為好友後，請透過語音或當面與 {name} 核對安全碼。只新增你認識的人。",
        M::AlreadyFriends => "你和 {name} 已經是好友了。",
        M::Added => "{name} 已在你的好友清單中。對方確認前不會分享任何內容。",
        M::NeedNameFirst => "請先選擇你的名稱：好友會在你的回覆中看到它。",
        M::NameTitle => "你的名稱",
        M::NameBody => "好友會看到這個名稱。如果你修改了它，好友的啟動器下次查看時會看到新名稱。",
        M::NamePlaceholder => "你的名稱",
        M::NameEmpty => "請選擇一個好友會看到的名稱。",
        M::VerifyTitle => "與 {name} 核對安全碼",
        M::VerifyBody => {
            "打電話給 {name} 或當面見面，互相念出你們的安全碼。如果相符，表示你們成為好友時沒有人從中冒充。不要在傳送連結的那個聊天裡核對。"
        }
        M::YourCode => "你的安全碼",
        M::TheirCode => "{name} 的安全碼",
        M::CodesMatch => "安全碼相符",
        M::RenameTitle => "重新命名 {name}",
        M::RenameBody => "只有你能看到這個名稱。對方自己選擇的名稱是 {name}。",
        M::NicknamePlaceholder => "暱稱",
        M::UseTheirName => "使用對方的名稱",
        M::Retire => "停用身分",
        M::RetireTitle => "要停用你的好友身分嗎？",
        M::RetireBody => {
            "你的每位好友都會收到此身分已停用的通知，他們會移除你，並不再信任你的伺服器上的模組。接著，你的金鑰、好友清單和尚未使用的好友連結會從這台電腦上刪除。如果你的金鑰可能遭竊，或想重新開始，就這樣做。"
        }
        M::RetireWarning => {
            "此操作無法復原。要再與任何人成為好友，你需要建立新身分，並用新的好友連結重新新增他們。"
        }
        M::RetireUnderstood => "我了解：所有好友都會被移除，且此操作無法復原",
        M::RetireConfirm => "停用身分",
        M::SharingFrom => "從 {core} 共享",
        M::RetireCore => "停用",
        M::RetireCoreTitle => "要停用 {name} 嗎？",
        M::RetireCoreBody => {
            "你的好友會被告知不再使用 {name}：他們不再看到它的伺服器，也不再信任其中的模組。電腦遺失、遭竊，或你不再從它共享時，請這麼做。"
        }
        M::RetireCoreWarning => {
            "此操作無法復原。即使此核心再次同步，好友也不會再看到它。要再次從那台電腦共享，它需要新的好友憑證：那台電腦上的 Lodestone Host 會在伺服器設定的「好友」區塊提供。"
        }
        M::RetireCoreConfirm => "停用核心",
        M::NameLike => "與 {names} 同名：請確認對方是誰",
        M::RemoveTitle => "要移除 {name} 嗎？",
        M::RemoveBody => {
            "你將看不到對方的伺服器，對方也看不到你的。Lodestone Host 下次更新你的核心時，它們會停止與對方分享。要重新成為好友，需要其中一人建立新的好友連結。"
        }
        M::RemoveFriend => "移除好友",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AnsweredTimes => {
            ["這個連結收到了 {count} 個回覆。可能有其他人拿到了這個連結：接受前請核對安全碼。"; 2]
        }
    }
}
