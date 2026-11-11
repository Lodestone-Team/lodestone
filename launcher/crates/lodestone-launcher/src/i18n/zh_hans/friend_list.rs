use crate::i18n::{FriendListMsg as M, FriendListPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Title => "好友",
        M::Hint => "只需添加一次的好友。无需链接即可查看并加入他们与你共享的服务器。",
        M::AddFriend => "添加好友",
        M::Refreshing => "正在查看好友状态…",
        M::You => "你",
        M::YourSafetyCode => "你的安全码：{code}",
        M::NoNameYet => "先选择好友会看到的名字，然后用好友链接添加他们。",
        M::ChooseName => "选择名字",
        M::EditName => "修改名字",
        M::MinecraftNameNote => "你的 Minecraft 名字",
        M::OpenFailed => "无法打开你的好友列表",
        M::ActionFailed => "无法更新你的好友列表",
        M::NoFriendsYet => "还没有好友。创建一个好友链接，发给你认识的人。",
        M::RefreshProblem => "有些好友无法查看：{error}",
        M::LinksTitle => "你的好友链接",
        M::LinkWaiting => "正在等待回复。仅可使用一次，有效期至 {time}。",
        M::CancelLink => "取消链接",
        M::CheckAnswers => "查看回复",
        M::ReplyFrom => "{name} 回复了你的好友链接",
        M::SafetyCode => "安全码：{code}",
        M::SameNameAs => "你已经有一位叫 {names} 的好友。继续之前，请确认这是另一个人。",
        M::Accept => "接受",
        M::Ignore => "忽略",
        M::WaitingFor => "正在等待 {name} 确认",
        M::NotVerified => "未验证",
        M::Verified => "安全码一致",
        M::Hidden => "已对其隐藏",
        M::Retired => "已停用此好友列表",
        M::TheirName => "对方的名字：{name}",
        M::OnlineOn => "在线：{cores}",
        M::NotSharingNow => "目前没有与你共享",
        M::SharesNothing => "尚未与你共享任何服务器",
        M::Servers => "服务器",
        M::CompareCodes => "核对安全码",
        M::HideFromThem => "对其隐藏",
        M::StopHiding => "取消隐藏",
        M::CoreOnline => "在线",
        M::CoreOffline => "目前未共享",
        M::LoadingServers => "正在获取服务器…",
        M::NoServersShared => "这台电脑上没有与你共享的内容。",
        M::ServersFailed => "无法列出服务器：{error}",
        M::ShowServers => "显示服务器",
        M::AddTitle => "添加好友",
        M::SendLinkTitle => "给对方发送好友链接",
        M::SendLinkBody => {
            "创建一个链接并发给你的好友。对方打开后，回复会显示在服务器页面上，等你接受。"
        }
        M::MakeLink => "创建链接",
        M::LinkWorksOnce => {
            "仅可使用一次，24 小时内有效。任何拿到这个链接的人都能请求成为你的好友，所以只发给对方本人。"
        }
        M::OpenLinkTitle => "或者打开对方的好友链接",
        M::LinkPlaceholder => "好友链接（https://connect.lodestone.cc/f/…）",
        M::LookUp => "查看",
        M::NotAFriendLink => "这不是好友链接。好友链接以 https://connect.lodestone.cc/f/ 开头。",
        M::PasteFirst => "请先粘贴好友链接。",
        M::OtherService => {
            "这个链接属于 {host}，不是此启动器使用的 Lodestone Connect 服务，因此无法在这里打开。"
        }
        M::PreviewFrom => "这个链接来自 {name}",
        M::PreviewCompare => "成为好友后，请通过语音或当面与 {name} 核对安全码。只添加你认识的人。",
        M::AlreadyFriends => "你和 {name} 已经是好友了。",
        M::PlaysAs => {
            "对方的启动器显示其 Minecraft 玩家名为 {name}。Lodestone 不会核实这一点：只有安全码才能证明确实是对方本人。"
        }
        M::Added => "{name} 已在你的好友列表中。对方确认前不会共享任何内容。",
        M::NeedNameFirst => "请先选择你的名字：好友会在你的回复中看到它。",
        M::NameTitle => "你的名字",
        M::NameBody => "好友会看到这个名字。如果你修改了它，好友的启动器下次查看时会看到新名字。",
        M::NamePlaceholder => "你的名字",
        M::NameEmpty => "请选择一个好友会看到的名字。",
        M::UseMinecraftName => "使用我的 Minecraft 名字",
        M::VerifyTitle => "与 {name} 核对安全码",
        M::VerifyBody => {
            "给 {name} 打电话或当面见面，互相念出你们的安全码。如果一致，说明你们成为好友时没有人从中冒充。不要在发送链接的那个聊天里核对。"
        }
        M::YourCode => "你的安全码",
        M::TheirCode => "{name} 的安全码",
        M::CodesMatch => "安全码一致",
        M::RenameTitle => "重命名 {name}",
        M::RenameBody => "只有你能看到这个名字。对方自己选择的名字是 {name}。",
        M::NicknamePlaceholder => "昵称",
        M::UseTheirName => "使用对方的名字",
        M::Retire => "停用身份",
        M::RetireTitle => "要停用你的好友身份吗？",
        M::RetireBody => {
            "你的每位好友都会收到此身份已停用的通知，他们会移除你，并不再信任你的服务器上的模组。随后，你的密钥、好友列表和未使用的好友链接会从这台电脑上删除。如果你的密钥可能被盗，或者想重新开始，就这样做。"
        }
        M::RetireWarning => {
            "此操作无法撤销。要再与任何人成为好友，你需要创建新身份，并用新的好友链接重新添加他们。"
        }
        M::RetireUnderstood => "我明白：所有好友都会被移除，且此操作无法撤销",
        M::RetireConfirm => "停用身份",
        M::SharingFrom => "从 {core} 共享",
        M::RetireCore => "停用",
        M::RetireCoreTitle => "要停用 {name} 吗？",
        M::RetireCoreBody => {
            "你的好友会被告知不再使用 {name}：他们不再看到它的服务器，也不再信任其中的模组。电脑丢失、被盗，或你不再从它共享时，请这样做。"
        }
        M::RetireCoreWarning => {
            "此操作无法撤销。即使此主机再次同步，好友也不会再看到它。要再次从那台电脑共享，它需要新的好友证书：那台电脑上的 Lodestone Host 会在服务器设置的“好友”部分提供。"
        }
        M::RetireCoreConfirm => "停用主机",
        M::NameLike => "与 {names} 同名：请确认对方是谁",
        M::RemoveTitle => "要移除 {name} 吗？",
        M::RemoveBody => {
            "你将看不到对方的服务器，对方也看不到你的。你的主机下次同步时，会停止与对方共享。要重新成为好友，需要其中一人创建新的好友链接。"
        }
        M::RemoveFriend => "移除好友",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AnsweredTimes => {
            ["这个链接收到了 {count} 个回复。可能有其他人拿到了这个链接：接受前请核对安全码。"; 2]
        }
    }
}
