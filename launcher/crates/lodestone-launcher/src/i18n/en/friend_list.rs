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
        /// The key of this person's safety code, such as K7Q2-M9XD-4TRW-H3BE, in a field to copy.
        YourSafetyCode = "Your safety code",
        NoNameYet = "Choose the name your friends will see, then add them with a friend link.",
        ChooseName = "Choose name",
        EditName = "Edit name",
        /// After the person's name in their own row, when it is their Minecraft
        /// account's player name (they didn't choose another).
        MinecraftNameNote = "your Minecraft name",
        OpenFailed = "Couldn't open your friend list",
        ActionFailed = "Couldn't update your friend list",
        NoFriendsYet = "No friends yet. Make a friend link and send it to someone you know.",
        /// Over the friends that couldn't be checked, each with what went wrong.
        RefreshProblem = "Some friends couldn't be checked",

        // ===== Friend links and their answers =====
        LinksTitle = "Your friend links",
        /// The badge on an open friend link.
        LinkWaiting = "Waiting for an answer",
        /// The key of how many times a friend link works.
        LinkUses = "Uses",
        LinkUsesOnce = "Once",
        CancelLink = "Cancel link",
        CheckAnswers = "Check for answers",
        /// Over the name of someone who answered a friend link.
        ReplyFrom = "Answered your friend link",
        /// The key of someone's safety code.
        SafetyCode = "Safety code",
        /// The key of the player name someone's launcher says they play Minecraft as.
        MinecraftName = "Minecraft name",
        /// Under the friends with the same name as someone who answered or sent a link.
        SameNameAs = "You already have a friend with this name. Make sure this is someone else before you go on.",
        /// The tag before the friends whose name is the same as someone's.
        SameNameTag = "Same name as",
        Accept = "Accept",
        Ignore = "Ignore",

        // ===== Friends =====
        NotVerified = "Not verified",
        /// The badge under a friend who hasn't confirmed yet.
        WaitingToConfirm = "Waiting to confirm",
        Verified = "Codes match",
        Hidden = "Hidden from them",
        Retired = "Stopped using this friend list",
        /// The key of the name a friend chose, under the nickname given to them.
        TheirName = "Their name",
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
        /// Over the person who sent a friend link.
        PreviewFrom = "Friend link from",
        PreviewCompare = "Once you're friends, compare safety codes with them by voice or in person. Only add people you know.",
        /// The badge on the person who sent a friend link, when they are a friend already.
        AlreadyFriends = "Already friends",
        /// Beside the face and the Minecraft name a friend link's sender (or someone who
        /// answered) says they play as. Nothing checks it, so it must not read as proof of who they are.
        PlaysAs = "Their launcher says they play Minecraft with this name. Lodestone doesn't check this: only the safety code shows it's really them.",
        /// Under the friend just added.
        Added = "In your friend list. Nothing is shared until they confirm.",
        NeedNameFirst = "Choose your name first: your friend sees it with your answer.",

        // ===== Your name =====
        NameTitle = "Your name",
        NameBody = "Friends see this name. If you change it, they see the new one the next time their launcher checks.",
        NamePlaceholder = "Your name",
        NameEmpty = "Choose a name your friends will see.",
        /// In the Your name dialog: go back to the Minecraft account's player name.
        UseMinecraftName = "Use my Minecraft name",

        // ===== Compare codes =====
        /// The friend is in a card under it.
        VerifyTitle = "Compare safety codes",
        VerifyBody = "Call them or meet them, and read your codes to each other. If they match, nobody came between you when you became friends. Don't compare them over the chat the link went through.",
        YourCode = "Your code",
        /// The key of a friend's safety code.
        TheirCode = "Their code",
        CodesMatch = "Codes match",

        // ===== Rename =====
        /// The friend is in a card under it.
        RenameTitle = "Rename friend",
        /// The name they chose is under it, keyed TheirName.
        RenameBody = "Only you see this name.",
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
        /// Under "You": the key of a computer (host) friends see this person share from.
        SharingFrom = "Sharing from",
        /// The button beside one of those hosts.
        RetireCore = "Retire",
        /// The host is in a card under it.
        RetireCoreTitle = "Retire host?",
        RetireCoreBody = "Your friends are told to drop this host: they stop seeing its servers and stop trusting their mods. Do this for a computer that was lost or stolen, or that you no longer share from.",
        RetireCoreWarning = "This can't be undone. Friends are never shown this host again, even if it syncs. To share from that computer again, it needs a new friend certificate: Lodestone Host on that computer offers one in a server's Settings, under Friends.",
        RetireCoreConfirm = "Retire host",
        /// Under a friend whose name looks like other friends', after a SameNameTag and their names.
        NameLikeBody = "Make sure you know who this is.",

        // ===== Remove =====
        /// The friend is in a card under it.
        RemoveTitle = "Remove friend?",
        RemoveBody = "You stop seeing their servers, and they stop seeing yours. Your hosts stop sharing with them the next time they sync. To be friends again, one of you makes a new friend link. Invite links you sent them keep working until you make new ones in Lodestone Host.",
        RemoveFriend = "Remove friend",

        // ===== LAN worlds =====
        /// The switch under "You" that shares the person's LAN worlds.
        ShareLan = "Share LAN worlds with friends",
        ShareLanBody = "When you open a singleplayer world to LAN, your friends see it here and can join it through Lodestone Connect while it's open. Friends you hide from don't see it.",
        /// While sharing is on and no world is open to LAN.
        LanNoneOpen = "No world is open to LAN right now. In the game, open a world and choose Open to LAN.",
        /// The key of a world this person has open to LAN, as the game announces it.
        LanSharing = "Shared with your friends",
        LanStarting = "Getting ready to share…",
        /// {error} is what went wrong.
        LanProblem = "Couldn't share your LAN worlds: {error}",
        LanNoList = "Add a friend first: only people on your friend list see your LAN worlds.",
        /// Under the person's own shared world, when they play with a Microsoft account.
        LanOnlineMode = "Friends need a Microsoft account to join: a LAN world uses your game's online mode.",
        /// Under "You": this launcher, which friends see sharing LAN worlds.
        LanFromHere = "Sharing LAN worlds from this launcher",
        /// Over a friend's LAN worlds (their launcher, as one of their hosts).
        LanWorlds = "LAN worlds",
        /// The tag on a friend's world opened to LAN.
        LanWorld = "LAN world",
        /// After a friend's LAN world's version: it can be joined now.
        OpenNow = "open now",
        /// Under a friend who has a world open to LAN.
        LanOpenStatus = "Has a LAN world open",
        /// Under a friend's LAN world.
        LanJoinNote = "Joinable while their game has it open.",
        LanNeedsMicrosoft = "Needs a Microsoft account: the world is in online mode.",
        /// Under a friend's LAN world that runs a mod loader (its row shows which).
        LanModded = "You need the same mods to join.",
    }

    pub enum FriendListPlural {
        /// Under an answer to a link more than one person answered.
        AnsweredTimes = (
            "This link was answered {count} time. Someone else may have this link: compare safety codes before you accept.",
            "This link was answered {count} times. Someone else may have this link: compare safety codes before you accept."
        ),
    }
}
