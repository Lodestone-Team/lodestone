//! The friend list on the Servers page: people added once with a friend link, whose
//! servers the launcher shows and joins without links (launcher/docs/friends.md).
//!
//! A "friend link" is the link that adds a friend; an "invite link" (the Servers
//! area's) joins one server. A "safety code" is the code two friends compare by voice
//! or in person.

crate::i18n::messages! {
    pub enum FriendListMsg {
        // ===== The section =====
        Title = "Friends",
        Hint = "People you added once. See and join the servers they share with you, without links.",
        AddFriend = "Add friend",
        /// The section's status while it asks the service about friends.
        Refreshing = "Checking on your friends…",
        /// The person's own row.
        You = "You",
        /// {code} is a safety code, such as K7Q2-M9XD-4TRW-H3BE.
        YourSafetyCode = "Your safety code: {code}",
        NoNameYet = "Choose the name your friends will see, then add them with a friend link.",
        ChooseName = "Choose name",
        EditName = "Edit name",
        /// After the person's name in their own row, when it is their Minecraft
        /// account's player name (they didn't choose another).
        MinecraftNameNote = "your Minecraft name",
        OpenFailed = "Couldn't open your friend list",
        ActionFailed = "Couldn't update your friend list",
        NoFriendsYet = "No friends yet. Make a friend link and send it to someone you know.",
        /// Problems while checking on friends: {error} is what went wrong.
        RefreshProblem = "Some friends couldn't be checked: {error}",

        // ===== Friend links and their answers =====
        LinksTitle = "Your friend links",
        /// {time} is when the link stops working, such as "Tomorrow at 14:05".
        LinkWaiting = "Waiting for an answer. Works once, until {time}.",
        CancelLink = "Cancel link",
        CheckAnswers = "Check for answers",
        /// {name} is the name the person chose.
        ReplyFrom = "{name} answered your friend link",
        /// {code} is a safety code.
        SafetyCode = "Safety code: {code}",
        /// {names} is one or more friends' names.
        SameNameAs = "You already have a friend called {names}. Make sure this is someone else before you go on.",
        Accept = "Accept",
        Ignore = "Ignore",

        // ===== Friends =====
        /// Under a friend who hasn't confirmed yet; {name} is their name.
        WaitingFor = "Waiting for {name} to confirm",
        NotVerified = "Not verified",
        Verified = "Codes match",
        Hidden = "Hidden from them",
        Retired = "Stopped using this friend list",
        /// {name} is the name the friend chose, under the nickname given to them.
        TheirName = "Their name: {name}",
        /// {cores} is a list of the friend's computers (host names).
        OnlineOn = "Online: {cores}",
        /// A friend whose hosts don't answer: off, or sharing nothing with this person
        /// (a host only answers friends it shares something with).
        NotSharingNow = "Not sharing with you right now",
        SharesNothing = "Not sharing any server with you yet",
        Servers = "Servers",
        CompareCodes = "Compare codes",
        HideFromThem = "Hide from them",
        StopHiding = "Stop hiding",
        CoreOnline = "Online",
        /// One of a friend's hosts that doesn't answer (see NotSharingNow).
        CoreOffline = "Not sharing now",
        LoadingServers = "Asking for the servers…",
        NoServersShared = "Nothing on this computer is shared with you.",
        /// {error} is what went wrong.
        ServersFailed = "Couldn't list the servers: {error}",
        ShowServers = "Show servers",

        // ===== Add a friend =====
        AddTitle = "Add a friend",
        SendLinkTitle = "Send them a friend link",
        SendLinkBody = "Make a link and send it to your friend. When they open it, their answer shows up on the Servers page for you to accept.",
        MakeLink = "Make a link",
        LinkWorksOnce = "Works once, for 24 hours. Anyone with this link can ask to be your friend, so send it only to them.",
        OpenLinkTitle = "Or open their friend link",
        LinkPlaceholder = "Friend link (https://connect.lodestone.cc/f/…)",
        LookUp = "Look up",
        NotAFriendLink = "That isn't a friend link. Friend links start with https://connect.lodestone.cc/f/.",
        PasteFirst = "Paste a friend link first.",
        /// {host} is the address in the link.
        OtherService = "This link is for {host}, which isn't the Lodestone Connect service this launcher uses, so it can't be opened here.",
        /// {name} is the name the link's sender chose.
        PreviewFrom = "This link is from {name}",
        PreviewCompare = "Once you're friends, compare safety codes with {name} by voice or in person. Only add people you know.",
        AlreadyFriends = "You're already friends with {name}.",
        /// Beside the skin of the Minecraft profile a friend link's sender (or someone
        /// who answered) says they play as. {name} is that profile's player name.
        /// Nothing checks it, so it must not read as proof of who they are.
        PlaysAs = "Their launcher says they play Minecraft as {name}. Lodestone doesn't check this: only the safety code shows it's really them.",
        Added = "{name} is in your friend list. Nothing is shared until they confirm.",
        NeedNameFirst = "Choose your name first: your friend sees it with your answer.",

        // ===== Your name =====
        NameTitle = "Your name",
        NameBody = "Friends see this name. If you change it, they see the new one the next time their launcher checks.",
        NamePlaceholder = "Your name",
        NameEmpty = "Choose a name your friends will see.",
        /// In the Your name dialog: go back to the Minecraft account's player name.
        UseMinecraftName = "Use my Minecraft name",

        // ===== Compare codes =====
        /// {name} is the friend's name.
        VerifyTitle = "Compare safety codes with {name}",
        VerifyBody = "Call {name} or meet them, and read your codes to each other. If they match, nobody came between you when you became friends. Don't compare them over the chat the link went through.",
        YourCode = "Your code",
        /// {name} is the friend's name.
        TheirCode = "{name}'s code",
        CodesMatch = "Codes match",

        // ===== Rename =====
        /// {name} is the friend's name.
        RenameTitle = "Rename {name}",
        RenameBody = "Only you see this name. The name they chose is {name}.",
        NicknamePlaceholder = "Nickname",
        UseTheirName = "Use their name",

        // ===== Retire =====
        /// The button in "You" that ends this person's friend identity.
        Retire = "Retire identity",
        RetireTitle = "Retire your friend identity?",
        RetireBody = "Each of your friends is told that this identity is retired, so they drop you and stop trusting your servers' mods. Then your key, your friend list and your open friend links are deleted from this computer. Do this if your key may have been stolen, or to start over.",
        RetireWarning = "This can't be undone. To be friends with anyone again, you make a new identity and add them again with new friend links.",
        RetireUnderstood = "I understand: all my friends are removed, and this can't be undone",
        RetireConfirm = "Retire identity",
        /// Under "You": a computer (host) friends see this person share from; {core} is its name.
        SharingFrom = "Sharing from {core}",
        /// The button beside one of those hosts.
        RetireCore = "Retire",
        /// {name} is the host's name.
        RetireCoreTitle = "Retire {name}?",
        RetireCoreBody = "Your friends are told to drop {name}: they stop seeing its servers and stop trusting their mods. Do this for a computer that was lost or stolen, or that you no longer share from.",
        RetireCoreWarning = "This can't be undone. Friends are never shown this host again, even if it syncs. To share from that computer again, it needs a new friend certificate: Lodestone Host on that computer offers one in a server's Settings, under Friends.",
        RetireCoreConfirm = "Retire host",
        /// Under a friend whose name looks like another friend's; {names} is one or more names.
        NameLike = "Same name as {names}: make sure you know who this is",

        // ===== Remove =====
        /// {name} is the friend's name.
        RemoveTitle = "Remove {name}?",
        RemoveBody = "You stop seeing their servers, and they stop seeing yours. Your hosts stop sharing with them the next time they sync. To be friends again, one of you makes a new friend link.",
        RemoveFriend = "Remove friend",
    }

    pub enum FriendListPlural {
        /// Under an answer to a link more than one person answered.
        AnsweredTimes = (
            "This link was answered {count} time. Someone else may have this link: compare safety codes before you accept.",
            "This link was answered {count} times. Someone else may have this link: compare safety codes before you accept."
        ),
    }
}
