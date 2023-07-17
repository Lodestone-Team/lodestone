use fancy_regex::Regex;
use lazy_static::lazy_static;

pub struct PlayerMessage {
    pub player: String,
    pub message: String,
}

pub fn parse_system_msg(msg: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\[.+\]+: (?!<)(.+)").unwrap();
    }
    if RE.is_match(msg).ok()? {
        RE.captures(msg)
            .ok()?
            .map(|caps| caps.get(1).unwrap().as_str().to_string())
    } else {
        None
    }
}

pub fn parse_player_msg(msg: &str) -> Option<PlayerMessage> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\[.+\]+: <(.+)> (.+)").unwrap();
    }
    if RE.is_match(msg).unwrap() {
        if let Some(cap) = RE.captures(msg).ok()? {
            Some(PlayerMessage {
                player: cap.get(1)?.as_str().to_string(),
                message: cap.get(2)?.as_str().to_string(),
            })
        } else {
            None
        }
    } else {
        None
    }
}

pub fn parse_player_joined(system_msg: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(.+) joined the game").unwrap();
    }
    if RE.is_match(system_msg).unwrap() {
        if let Some(cap) = RE.captures(system_msg).ok()? {
            Some(cap.get(1)?.as_str().to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub fn parse_player_left(system_msg: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(.+) left the game").unwrap();
    }
    if RE.is_match(system_msg).unwrap() {
        if let Some(cap) = RE.captures(system_msg).ok()? {
            Some(cap.get(1)?.as_str().to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub fn parse_server_started(system_msg: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"Done \(.+\)!"#).unwrap();
    }
    RE.is_match(system_msg).unwrap()
}
