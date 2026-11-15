//! The generic rule against credentials on top of the named lists (`export::is_way_in`,
//! `export::SERVER_PRIVATE`, `import::is_permissions_data`): a settings file that holds a
//! password, a token or a key under a name that says so never leaves the computer and never
//! reaches a server made from a file, whichever mod or plugin wrote it. See "M6a
//! decisions", "Credentials found by their names".
//!
//! - [`scanned_format`] says which files are read: every file with a settings extension
//!   ([`SCANNED_EXTENSIONS`]) outside the worlds (and in a world's `serverconfig/`), and a
//!   file with no extension in a settings folder ([`in_settings_folder`]). `server.properties`
//!   and the YAML files an export blanks are left to their precise cleaning.
//! - [`find_credential`] reads one: JSON, JSON5, JSONC and SNBT by their tokens
//!   ([`Format::Json`]), every other format line by line ([`Format::Lines`]: YAML, TOML, INI,
//!   `.properties`, HOCON, `.cfg`, `.env`, text), following the nesting of indentation,
//!   braces, brackets and `[sections]`, so a key is judged with the key above it. A key whose
//!   words name a credential ([`credential_key`]) with a value that isn't a placeholder
//!   ([`real_value`]) is one, and so is any value that is a login by its shape
//!   ([`credential_value`]: a URL with a password, a Discord webhook, a Discord bot token).
//!   A file it can't follow is still read line by line, which errs towards finding one.
//! - [`withheld_by_name`] names files that are a credential with no keys at all (`.token`,
//!   `*.pem`, `*.key`, `id_rsa`, `.env`, `credentials.json`, `token.txt`, `secret*.txt`).
//! - [`too_large`] withholds a file the scan would read, or any file in a settings folder
//!   that isn't plainly data, over [`MAX_SCANNED_FILE`].

use lodestone_image::paths::same_name;
use lodestone_image::CanonicalPath;

/// The largest file the scan reads; a file it would read that is bigger is withheld.
pub const MAX_SCANNED_FILE: u64 = 4 << 20;

/// The extensions of the settings files the scan reads (any letter case).
pub const SCANNED_EXTENSIONS: &[&str] = &[
    "toml",
    "yml",
    "yaml",
    "json",
    "json5",
    "jsonc",
    "properties",
    "conf",
    "cfg",
    "hocon",
    "ini",
    "env",
    "snbt",
    "txt",
];

/// The extensions read by their tokens ([`Format::Json`]).
const JSON_EXTENSIONS: &[&str] = &["json", "json5", "jsonc", "snbt"];

/// Extensions that are plainly data, never settings: a file of one in a settings folder
/// over [`MAX_SCANNED_FILE`] still travels.
pub const DATA_EXTENSIONS: &[&str] = &[
    "jar",
    "zip",
    "gz",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "ogg",
    "wav",
    "mp3",
    "nbt",
    "dat",
    "dat_old",
    "mca",
    "mcr",
    "mcc",
    "schem",
    "schematic",
    "litematic",
    "ttf",
    "otf",
    "db",
    "sqlite",
    "sqlite3",
];

/// The extensions of key and token files, a credential by their names
/// ([`withheld_by_name`]).
const KEY_EXTENSIONS: &[&str] = &[
    "token", "key", "pem", "p12", "pfx", "jks", "keystore", "secret", "kdbx",
];

/// What can follow a settings file's extension in the name of a copy of it
/// ([`underlying_name`]): `config.yml-build.1.28.0.old`, `config.json.bak`,
/// `settings.toml~`, `config.yml_2024`, `config.yml (1)`.
const COPY_SEPARATORS: &[char] = &['-', '_', '.', '~', ' ', '(', '+', ','];

/// How [`find_credential`] reads a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// By its tokens: JSON, JSON5, JSONC, SNBT.
    Json,
    /// Line by line: YAML, TOML, INI, `.properties`, HOCON, `.cfg`, `.env`, text.
    Lines,
}

fn folded(text: &str) -> String {
    text.to_uppercase().to_lowercase()
}

/// A file name's extension, folded, when it has one (`.token` has none: it is a name).
fn extension(name: &str) -> Option<String> {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => Some(folded(ext)),
        _ => None,
    }
}

/// The name of the settings or key file a copy named `name` (folded) was made from, when
/// its own extension is neither read, a key's nor plainly data: the name up to the first
/// settings extension ([`SCANNED_EXTENSIONS`]) or key extension ([`KEY_EXTENSIONS`]) that a
/// separator follows ([`COPY_SEPARATORS`]) or that ends the name. DiscordSRV moves its
/// settings to `config.yml-build.<version>.old` when it updates them, token and all; other
/// copies are `config.json.bak`, `bot.token.old`, `settings.toml~`, `config.yml.disabled`.
/// `name` itself otherwise.
fn underlying_name(name: &str) -> &str {
    let known = |ext: &str| {
        SCANNED_EXTENSIONS.contains(&ext)
            || KEY_EXTENSIONS.contains(&ext)
            || DATA_EXTENSIONS.contains(&ext)
    };
    if extension(name).is_none_or(|ext| known(&ext)) {
        return name;
    }
    for (dot, _) in name.match_indices('.') {
        let after = &name[dot + 1..];
        for ext in SCANNED_EXTENSIONS.iter().chain(KEY_EXTENSIONS) {
            let Some(rest) = after.strip_prefix(ext) else {
                continue;
            };
            if rest.is_empty() || rest.starts_with(COPY_SEPARATORS) {
                return &name[..dot + 1 + ext.len()];
            }
        }
    }
    name
}

/// Whether `parts` are in a world's folder (`saves/<world>/…`).
fn in_world(parts: &[&str]) -> bool {
    parts.len() >= 3 && same_name(parts[0], "saves")
}

/// Whether the file at `path` is in a folder of settings: `config/`, `plugins/`,
/// `defaultconfigs/`, a folder in `mods/` (`mods/tebex/`) or a world's `serverconfig/`.
pub fn in_settings_folder(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [top, _, ..]
            if lodestone_image::paths::in_names(top, &["config", "plugins", "defaultconfigs"]) =>
        {
            true
        }
        [top, _, _, ..] if same_name(top, "mods") => true,
        [top, _, dir, _, ..] if same_name(top, "saves") => same_name(dir, "serverconfig"),
        _ => false,
    }
}

/// How the scan reads the file at `path`, when it reads it: a file with a settings
/// extension outside the worlds or in a world's `serverconfig/`, or a file with no extension
/// in a settings folder. `None` for `server.properties` and the YAML files an export
/// blanks (`export::is_cleaned`), which are cleaned key by key instead.
pub fn scanned_format(path: &CanonicalPath) -> Option<Format> {
    let parts: Vec<&str> = path.components().collect();
    let name = parts.last()?;
    if in_world(&parts) && !(parts.len() >= 4 && same_name(parts[2], "serverconfig")) {
        return None;
    }
    if super::export::is_cleaned(path) {
        return None;
    }
    let name = folded(name);
    match extension(underlying_name(&name)) {
        Some(ext) if JSON_EXTENSIONS.contains(&ext.as_str()) => Some(Format::Json),
        Some(ext) if SCANNED_EXTENSIONS.contains(&ext.as_str()) => Some(Format::Lines),
        None if in_settings_folder(path) => Some(Format::Lines),
        _ => None,
    }
}

/// Whether a file of `size` bytes at `path` is too big to check, so it is withheld: one the
/// scan would read, or any file in a settings folder whose extension isn't plainly data
/// ([`DATA_EXTENSIONS`]), over [`MAX_SCANNED_FILE`].
pub fn too_large(path: &CanonicalPath, size: u64) -> bool {
    if size <= MAX_SCANNED_FILE {
        return false;
    }
    if scanned_format(path).is_some() {
        return true;
    }
    let data = path
        .components()
        .last()
        .and_then(extension)
        .is_some_and(|ext| DATA_EXTENSIONS.contains(&ext.as_str()));
    in_settings_folder(path) && !data
}

/// Whether the file at `path` is a credential by its name, with no keys to read: a token
/// file (`.token`, `*.token`, DiscordSRV's), a key or keystore (`*.key`, `*.pem`, `*.p12`,
/// `*.pfx`, `*.jks`, `*.keystore`, `*.secret`, `*.kdbx`, `id_rsa*`, `id_ed25519*`,
/// `id_ecdsa*`, `id_dsa*`), an environment or login file (`.env*`, `.netrc`, `.pgpass`,
/// `.htpasswd`, `credentials`, `credentials.json`), a text file or a file with no extension
/// whose name says it holds one (`token.txt`, `bot-token.txt`, Plasmo Voice's
/// `forwarding-secret`) and `secret*.txt`. Anywhere but in a world's region, entity and
/// POI data.
pub fn withheld_by_name(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    let Some(name) = parts.last() else {
        return false;
    };
    if in_world(&parts)
        && parts
            .iter()
            .any(|part| lodestone_image::paths::in_names(part, &["region", "entities", "poi"]))
    {
        return false;
    }
    let name = folded(name);
    let name = underlying_name(&name).to_string();
    if [
        ".token",
        ".netrc",
        ".pgpass",
        ".htpasswd",
        "credentials",
        "credentials.json",
        "token.txt",
    ]
    .contains(&name.as_str())
        || name.starts_with(".env")
        || ["id_rsa", "id_ed25519", "id_ecdsa", "id_dsa"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    {
        return true;
    }
    let ext = extension(&name);
    let stem = match &ext {
        Some(ext) => &name[..name.len() - ext.len() - 1],
        None => name.as_str(),
    };
    match ext.as_deref() {
        Some(ext) if KEY_EXTENSIONS.contains(&ext) => true,
        Some("txt") => stem.starts_with("secret") || credential_key(None, stem),
        None => credential_key(None, stem),
        _ => false,
    }
}

/// `key` split into its words, folded: at separators (anything but a letter or a digit), at
/// camelCase (`botToken`: bot, token; `APIKey`: api, key) and between letters and digits.
pub fn words(key: &str) -> Vec<String> {
    let chars: Vec<char> = key.chars().collect();
    let mut out = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                out.push(folded(&word));
                word.clear();
            }
            continue;
        }
        if let Some(before) = word.chars().last() {
            let next = chars.get(i + 1).copied();
            let boundary = (before.is_lowercase() && c.is_uppercase())
                || (before.is_alphabetic() != c.is_alphabetic())
                || (before.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()));
            if boundary {
                out.push(folded(&word));
                word.clear();
            }
        }
        word.push(c);
    }
    if !word.is_empty() {
        out.push(folded(&word));
    }
    out
}

/// Words that name a credential when a key's name ends with them (`botToken`, `bottoken`,
/// `rcon.password`, `client_secret`, `discordWebhook`, `allowed-tokens`).
const CREDENTIAL_ENDINGS: &[&str] = &[
    "password",
    "passwords",
    "passwd",
    "passphrase",
    "secret",
    "secrets",
    "token",
    "tokens",
    "credential",
    "credentials",
    "webhook",
    "webhooks",
    "apikey",
    "apikeys",
];

/// Words that name a credential only as a whole word.
const CREDENTIAL_WORDS: &[&str] = &["pwd", "bearer", "dsn"];

/// The words before `key` that make it a credential (`api key`, `secret-key`, `license key`,
/// `serverKey`), also written as one word (`apikey`, `secretkey`).
const KEY_QUALIFIERS: &[&str] = &[
    "api",
    "secret",
    "private",
    "license",
    "licence",
    "access",
    "auth",
    "store",
    "server",
    "signing",
    "encryption",
    "client",
    "master",
    "shared",
    "hmac",
    "rcon",
    "bot",
    "webhook",
    "session",
    "sentry",
    "discord",
    "tebex",
    "buycraft",
    "steam",
    "github",
    "stripe",
    "paypal",
    "telegram",
    "slack",
];

/// The words before `pass` that make it a password (`db_pass`, `mysql-pass`, `rcon.pass`):
/// `pass` alone is not one (`passive`, a pass of a game).
const PASS_QUALIFIERS: &[&str] = &[
    "db",
    "database",
    "mysql",
    "mariadb",
    "sql",
    "postgres",
    "postgresql",
    "mongo",
    "mongodb",
    "redis",
    "rcon",
    "smtp",
    "mail",
    "email",
    "user",
    "admin",
    "login",
    "auth",
    "ftp",
    "sftp",
    "account",
    "server",
    "proxy",
];

/// Words that hold their parent's credential (`token: { value: … }`).
const VALUE_WORDS: &[&str] = &["value", "string", "text", "data", "raw", "content"];

/// Whether a key named `key`, under a key named `parent` (when there is one), names a
/// credential by its words ([`words`]): its last word is a credential (`password`,
/// `passwd`, `passphrase`, `secret`, `token`, `credentials`, `webhook`, `apikey`, `pwd`,
/// `bearer`, `dsn`, or a word ending with one: `bottoken`), or `key` after a word that makes
/// it one (`api key`, `secret-key`, `private_key`, `licenseKey`, `server-key`; the word
/// before may be the parent's last: `secret: { key: … }`), `pass` after one that makes it a
/// password (`db_pass`), `url` after `webhook`, or a value word (`value`) under a
/// credential. Never bare `key` or `pass`, nor a word that only begins like one
/// (`keyFileName`, `keepInventory`, `passive`, `tokenExpiry`, `secretRooms`).
pub fn credential_key(parent: Option<&str>, key: &str) -> bool {
    let own = words(key);
    let Some(last) = own.last() else {
        return false;
    };
    let parent_words = parent.map(words).unwrap_or_default();
    let before = if own.len() >= 2 {
        own.get(own.len() - 2).cloned()
    } else {
        parent_words.last().cloned()
    };
    let before = before.as_deref();
    let compound = |suffix: &str, qualifiers: &[&str]| {
        last.strip_suffix(suffix).is_some_and(|prefix| {
            qualifiers.contains(&prefix)
                || (prefix.is_empty() && before.is_some_and(|b| qualifiers.contains(&b)))
        })
    };
    CREDENTIAL_ENDINGS
        .iter()
        .any(|ending| last.ends_with(ending))
        || CREDENTIAL_WORDS.contains(&last.as_str())
        || compound("key", KEY_QUALIFIERS)
        || compound("keys", KEY_QUALIFIERS)
        || compound("pass", PASS_QUALIFIERS)
        || matches!(last.as_str(), "webhookurl" | "webhookuri")
        || (matches!(last.as_str(), "url" | "uri" | "link") && before == Some("webhook"))
        || (VALUE_WORDS.contains(&last.as_str())
            && own.len() == 1
            && parent.is_some_and(|parent| credential_key(None, parent)))
}

/// Values that say no credential was set (folded, letters and digits only).
const PLACEHOLDERS: &[&str] = &[
    "",
    "changeme",
    "changethis",
    "changeit",
    "replaceme",
    "none",
    "null",
    "nil",
    "undefined",
    "empty",
    "unset",
    "notset",
    "default",
    "example",
    "sample",
    "placeholder",
    "todo",
    "tbd",
    "na",
    "disabled",
    "secret",
    "secretkey",
    "password",
    "pass",
    "passwd",
    "token",
    "bottoken",
    "apikey",
    "key",
    "webhook",
    "webhookurl",
    "yourtoken",
    "yourkey",
    "yourpassword",
    "yoursecret",
    // passwords settings files ship with (HuskHomes' `pa55w0rd`)
    "pa55w0rd",
    "passw0rd",
    "p4ssw0rd",
    "password1",
    "password123",
    "defaultpass",
    "defaultpassword",
    "mypassword",
    "guest",
];

/// Words a placeholder's text names (`your-token-here`, `INSERT_BOT_TOKEN_HERE`).
const PLACEHOLDER_NAMES: &[&str] = &[
    "token", "key", "secret", "password", "pass", "webhook", "url", "id",
];

/// Whether `value`, a key's value as written (quotes taken off), is a real value rather
/// than a placeholder: not empty or blank, not a boolean (`true`, `false`, `yes`, `no`,
/// `on`, `off`), not a number of fewer than 8 digits (`maxTokens: 64`), not one character
/// repeated (`xxxxxx`, `******`), not a reference to something else (`${BOT_TOKEN}`,
/// `{{token}}`, `%token%`, `<token>`, `[]`, `{}`), not one of [`PLACEHOLDERS`]
/// (`changeme`, `none`, `null`, `TOKEN`, `password`) nor the key's own name or last word
/// (`BOT_TOKEN` for `botToken`), and not an instruction (`your-token-here`,
/// `INSERT TOKEN HERE`, `Paste your key`).
pub fn real_value(key: &str, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if ["true", "false", "yes", "no", "on", "off", "~"].contains(&lower.as_str()) {
        return false;
    }
    let digits = value.chars().filter(char::is_ascii_digit).count();
    let number = value
        .trim_start_matches(['+', '-'])
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        && digits > 0;
    if number && digits < 8 {
        return false;
    }
    let mut chars = value.chars();
    let first = chars.next().unwrap_or_default();
    if chars.all(|c| c == first) {
        return false;
    }
    let wrapped = |open: &str, close: &str| value.starts_with(open) && value.ends_with(close);
    if wrapped("${", "}")
        || wrapped("{{", "}}")
        || (wrapped("%", "%") && value.len() > 2)
        || wrapped("<", ">")
        || value == "[]"
        || value == "{}"
    {
        return false;
    }
    let normal: String = value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    if PLACEHOLDERS.contains(&normal.as_str()) {
        return false;
    }
    let key_words = words(key);
    if normal == key_words.concat() || key_words.last().is_some_and(|w| *w == normal) {
        return false;
    }
    let names_one = PLACEHOLDER_NAMES.iter().any(|name| normal.contains(name));
    let instruction = [
        "your", "insert", "enter", "paste", "put", "replace", "add", "set",
    ]
    .iter()
    .any(|verb| normal.starts_with(verb));
    !(normal.starts_with("your") || (names_one && (instruction || normal.ends_with("here"))))
}

/// Whether `value` is a login by its shape, whatever its key: a URL with a password in it
/// (`mongodb://user:password@host`, unless the password is a placeholder), a JDBC URL's
/// `password=` parameter, a Discord webhook URL with its token, or a Discord bot token.
pub fn credential_value(value: &str) -> bool {
    let value = value.trim();
    url_password(value) || query_password(value) || discord_webhook(value) || discord_token(value)
}

/// A URL's password (`scheme://user:password@host`), when it isn't a placeholder.
fn url_password(value: &str) -> bool {
    let Some(at) = value.find("://") else {
        return false;
    };
    let scheme = &value[..at];
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | ':'))
    {
        return false;
    }
    let rest = &value[at + 3..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let Some((userinfo, _)) = authority.rsplit_once('@') else {
        return false;
    };
    match userinfo.split_once(':') {
        Some((_, password)) => real_value("password", password),
        None => false,
    }
}

/// A `password=` parameter in a URL or connection string (JDBC's `?password=…`, `;password=…`).
fn query_password(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find("password=") {
        let at = from + found;
        let before = lower[..at].chars().last();
        if matches!(before, Some('?' | '&' | ';')) {
            let rest = &value[at + "password=".len()..];
            let password = rest.split(['&', ';']).next().unwrap_or_default();
            if real_value("password", password) {
                return true;
            }
        }
        from = at + 1;
    }
    false
}

/// A Discord webhook URL with its id and token
/// (`https://discord.com/api/webhooks/<id>/<token>`, `discordapp.com`, `canary.`, `ptb.`).
fn discord_webhook(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let Some(at) = lower.find("/api/webhooks/") else {
        return false;
    };
    let host = lower[..at].rsplit("//").next().unwrap_or_default();
    if !(host.ends_with("discord.com") || host.ends_with("discordapp.com")) {
        return false;
    }
    let mut parts = value[at + "/api/webhooks/".len()..].split('/');
    let id = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default();
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) && token.len() >= 20
}

/// A Discord bot token: three base64url parts joined by dots, the user id (beginning `M`,
/// `N` or `O`, 23 to 28 letters), a time (6 or 7) and an HMAC (27 or more).
fn discord_token(value: &str) -> bool {
    let token = value.trim_start_matches("Bot ").trim_start_matches("bot ");
    let parts: Vec<&str> = token.split('.').collect();
    let base64 = |part: &str| {
        part.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    matches!(parts.as_slice(), [id, time, hmac]
        if (23..=28).contains(&id.len())
            && id.starts_with(['M', 'N', 'O'])
            && (6..=7).contains(&time.len())
            && hmac.len() >= 27
            && parts.iter().all(|p| base64(p)))
}

/// The key path of the first credential `text` holds, read as `format` (see the module):
/// `parent.key`, for the log (never its value). `None` when it holds none.
pub fn find_credential(text: &str, format: Format) -> Option<String> {
    let ctx = Context::of(text, format);
    let ctx = &ctx;
    match format {
        Format::Json => scan_json(ctx, text, None).or_else(|| scan_lines(ctx, text)),
        Format::Lines => scan_lines(ctx, text),
    }
}

/// What a credential found is called: `parent.key`.
fn found(parent: Option<&str>, key: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}.{key}"),
        _ => key.to_string(),
    }
}

/// What the whole file says about its keys, read before they are judged ([`Context::of`]).
#[derive(Debug, Default)]
struct Context {
    /// Whether a key switches key authentication on (`useKeyAuth: true`, `auth-enabled:
    /// true`): a bare `key` or `pass` is then the key that lets its holder in (ServerTap's
    /// `key:`, which runs console commands through its REST API).
    key_auth: bool,
    /// The values of the keys naming a user or a database (`username: worldguard`), as
    /// [`normal`] writes them: a password that is one of them is a shipped default
    /// (WorldGuard's `password: worldguard`), not one an owner chose.
    names: Vec<String>,
}

/// `value` folded to its letters and digits, as placeholders are compared.
fn normal(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Whether a key's words name a user or a database (`username`, `user`, `db-name`,
/// `database`), whose value a default password repeats.
fn names_an_account(key: &str) -> bool {
    let own = words(key);
    let tail: Vec<&str> = own.iter().rev().take(2).rev().map(String::as_str).collect();
    matches!(
        tail.last().copied(),
        Some("user" | "username" | "login" | "database" | "db" | "schema" | "dbname")
    ) || matches!(tail.as_slice(), ["user" | "db" | "database", "name"])
}

/// Whether `value` switches something on: `true`, `yes`, `on`, `1`.
fn switched_on(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

impl Context {
    /// What `text`, read as `format`, says about its keys: every `key: value` (or `=`) on a
    /// line, and for JSON every key's scalar.
    fn of(text: &str, format: Format) -> Context {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if format == Format::Json {
            let tokens = json_tokens(text);
            for window in tokens.windows(3) {
                if let [Token::Text(key), Token::Punct(':' | '='), Token::Text(value)] = window {
                    pairs.push((key.clone(), value.clone()));
                }
            }
        }
        for line in text.lines() {
            let line = line.trim();
            let line = line.strip_prefix("- ").unwrap_or(line);
            let line = line.strip_prefix("export ").unwrap_or(line);
            if let Some((key, value)) = split_line(line) {
                let key = match key.as_bytes() {
                    [letter, b':', ..] if letter.is_ascii_alphabetic() => key[2..].to_string(),
                    _ => key,
                };
                pairs.push((key.trim_matches(['"', '\'']).to_string(), unquote(&value)));
            }
        }
        let mut ctx = Context::default();
        for (key, value) in pairs {
            if words(&key).iter().any(|w| w == "auth") && switched_on(&value) {
                ctx.key_auth = true;
            }
            if names_an_account(&key) {
                let value = normal(&value);
                if !value.is_empty() {
                    ctx.names.push(value);
                }
            }
        }
        ctx
    }
}

/// Whether `key` is a bare `key`, `keys` or `pass`, which names a credential only in a
/// file that switches key authentication on ([`Context::key_auth`]).
fn bare_key(key: &str) -> bool {
    matches!(words(key).as_slice(), [only] if matches!(only.as_str(), "key" | "keys" | "pass"))
}

/// Whether `value`, the value of a `dsn` key, is a database's address with no login in it
/// (`jdbc:mysql://localhost/worldguard`): no user or password before an `@` and no
/// `password=`. A Sentry DSN carries its key before an `@`, so it still counts.
fn plain_dsn(key: &str, value: &str) -> bool {
    words(key).last().is_some_and(|w| w == "dsn")
        && (value.contains("://") || value.starts_with("jdbc:"))
        && !value.contains('@')
        && !value.to_ascii_lowercase().contains("password=")
}

/// Whether the value `value` of the key `key` under `parent` is a credential: a key that
/// names one ([`credential_key`], or a bare `key` in a file with key authentication on)
/// with a real value ([`real_value`]) that doesn't repeat a user or database name
/// ([`Context::names`]) and isn't a database address with no login ([`plain_dsn`]), or a
/// value that is a login by its shape ([`credential_value`]).
fn judge(ctx: &Context, parent: Option<&str>, key: &str, value: &str) -> Option<String> {
    let named = credential_key(parent, key) || (ctx.key_auth && bare_key(key));
    let key_says = named
        && real_value(key, value)
        && !ctx.names.contains(&normal(value))
        && !plain_dsn(key, value);
    (key_says || credential_value(value)).then(|| found(parent, key))
}

/// A token of a JSON-like text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A quoted string's text, or a bare word (a key, a number, `true`).
    Text(String),
    Punct(char),
}

/// `text`'s tokens: strings (double or single quoted, with escapes; one left open runs to
/// the end), bare words and `{ } [ ] : , =`, comments (`//`, `/* */`, `#`) skipped.
fn json_tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') || c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
        } else if matches!(c, '{' | '}' | '[' | ']' | ':' | ',' | '=') {
            tokens.push(Token::Punct(c));
            i += 1;
        } else if c == '"' || c == '\'' {
            let mut text = String::new();
            i += 1;
            while i < chars.len() && chars[i] != c {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                text.push(chars[i]);
                i += 1;
            }
            i += 1;
            tokens.push(Token::Text(text));
        } else {
            let start = i;
            while i < chars.len()
                && !chars[i].is_whitespace()
                && !matches!(
                    chars[i],
                    '{' | '}' | '[' | ']' | ':' | ',' | '=' | '"' | '\''
                )
            {
                i += 1;
            }
            tokens.push(Token::Text(chars[start..i].iter().collect()));
        }
    }
    tokens
}

/// Reads JSON-like text (`text`, inside the key `outer` when it is a value) by its tokens:
/// a text followed by `:` (or `=`) is a key, `{` and `[` open its value, and each scalar is
/// judged with its key and the key above it; an array's items with the array's key.
fn scan_json(ctx: &Context, text: &str, outer: Option<&str>) -> Option<String> {
    #[derive(Debug)]
    struct Frame {
        /// The key whose value it is.
        owner: Option<String>,
        /// The key above that.
        parent: Option<String>,
        array: bool,
    }
    let tokens = json_tokens(text);
    let mut stack: Vec<Frame> = vec![Frame {
        owner: outer.map(str::to_string),
        parent: None,
        array: false,
    }];
    let mut pending: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        let top = stack.last();
        let (owner, parent, array) = top
            .map(|f| (f.owner.clone(), f.parent.clone(), f.array))
            .unwrap_or((None, None, false));
        match &tokens[i] {
            Token::Punct('{') | Token::Punct('[') => {
                let is_array = tokens[i] == Token::Punct('[');
                let (new_owner, new_parent) = match pending.take() {
                    Some(key) => (Some(key), owner.clone()),
                    None => (owner.clone(), parent.clone()),
                };
                stack.push(Frame {
                    owner: new_owner,
                    parent: new_parent,
                    array: is_array,
                });
            }
            Token::Punct('}') | Token::Punct(']') => {
                pending = None;
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            Token::Punct(_) => {}
            Token::Text(text) => {
                let is_key = !array
                    && pending.is_none()
                    && matches!(tokens.get(i + 1), Some(Token::Punct(':' | '=')));
                if is_key {
                    pending = Some(text.clone());
                    i += 2;
                    continue;
                }
                let (key, above) = match pending.take() {
                    Some(key) => (Some(key), owner.clone()),
                    None if array => (owner.clone(), parent.clone()),
                    None => (None, None),
                };
                let verdict = match &key {
                    Some(key) => judge(ctx, above.as_deref(), key, text),
                    None => credential_value(text).then(|| "value".to_string()),
                };
                if verdict.is_some() {
                    return verdict;
                }
            }
        }
        i += 1;
    }
    None
}

/// A value as written after its key: its quotes taken off (and what follows a closing
/// quote), or a trailing comment (` #`, ` ;`, ` //`) and comma taken off an unquoted one.
fn unquote(value: &str) -> String {
    let value = value.trim();
    if let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let mut out = String::new();
        let mut chars = value[1..].chars();
        while let Some(c) = chars.next() {
            if c == quote {
                return out;
            }
            if c == '\\' && quote == '"' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            out.push(c);
        }
        return out;
    }
    let mut end = value.len();
    for marker in [" #", "\t#", " ;", " //"] {
        if let Some(at) = value.find(marker) {
            end = end.min(at);
        }
    }
    value[..end].trim().trim_end_matches(',').trim().to_string()
}

/// Where a line's key ends: the first `=`, or the first `:` that isn't a URL's (`://`).
fn separator(line: &str) -> Option<usize> {
    line.char_indices()
        .find(|(i, c)| *c == '=' || (*c == ':' && !line[i + 1..].starts_with("//")))
        .map(|(i, _)| i)
}

/// Splits a line (indentation and any `- ` taken off) at its key: the key (quotes taken
/// off) and what follows the separator ([`separator`]); for a line with none, the first
/// space (`.properties`' `key value`). `None` when the line has no key.
fn split_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if let Some(quote) = line.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let close = line[1..].find(quote)? + 1;
        let key = line[1..close].to_string();
        let rest = line[close + 1..].trim_start();
        let rest = rest.strip_prefix([':', '='])?;
        return Some((key, rest.to_string()));
    }
    match separator(line) {
        Some(at) if at > 0 => Some((line[..at].trim().to_string(), line[at + 1..].to_string())),
        Some(_) => None,
        None => {
            let (key, value) = line.split_once([' ', '\t'])?;
            Some((key.to_string(), value.to_string()))
        }
    }
}

/// Whether a list item's text (after `- `) is a key and its value (`- name: a`) rather than
/// a scalar (`- abc`, `- https://x`).
fn keyed_item(item: &str) -> bool {
    let item = item.trim();
    if item.starts_with(['"', '\'']) {
        return split_line(item).is_some();
    }
    separator(item).is_some_and(|at| at > 0 && !item[..at].trim().contains(char::is_whitespace))
}

/// Reads a settings file line by line (see the module): YAML's indentation, TOML's and
/// INI's `[sections]`, HOCON's and `.cfg`'s braces, Forge's `.cfg` type letters (`S:`),
/// `.env`'s `export`, lists (`- item`, `[`…`]`, `.cfg`'s `<`…`>`) and multi-line strings
/// (YAML's block scalars and TOML's triple quotes, judged by their first line).
fn scan_lines(ctx: &Context, text: &str) -> Option<String> {
    #[derive(Debug)]
    struct Frame {
        indent: usize,
        key: String,
        /// Opened by a brace or bracket, closed by its partner rather than by indentation.
        closed_by: Option<char>,
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut stack: Vec<Frame> = Vec::new();
    let mut section: Option<String> = None;
    // a YAML block scalar's lines (more indented than this) or a TOML string's (to this)
    let mut skip_indent: Option<usize> = None;
    let mut skip_to: Option<&str> = None;
    let indentation = |line: &str| line.len() - line.trim_start().len();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        i += 1;
        let indent = indentation(line);
        let body = line.trim();
        if let Some(close) = skip_to {
            if body.contains(close) {
                skip_to = None;
            }
            continue;
        }
        if let Some(block) = skip_indent {
            if body.is_empty() || indent > block {
                continue;
            }
            skip_indent = None;
        }
        if body.is_empty()
            || body.starts_with('#')
            || body.starts_with(';')
            || body.starts_with("//")
            || body.starts_with('!')
        {
            continue;
        }
        // a list's items may sit at their key's indentation (`key:` then `- item`)
        let item = body == "-" || body.starts_with("- ");
        while stack.last().is_some_and(|f| {
            f.closed_by.is_none() && (f.indent > indent || (f.indent == indent && !item))
        }) {
            stack.pop();
        }
        // closing braces, brackets and `.cfg` lists
        if body.starts_with(['}', ']']) || body == ">" {
            for c in body
                .chars()
                .take_while(|c| matches!(c, '}' | ']' | '>' | ',' | ' '))
            {
                if matches!(c, '}' | ']' | '>') {
                    while let Some(frame) = stack.pop() {
                        if frame.closed_by.is_some() {
                            break;
                        }
                    }
                }
            }
            continue;
        }
        // a TOML or INI section: the parent of the keys after it
        if body.starts_with('[') && !stack.last().is_some_and(|f| f.closed_by == Some(']')) {
            let inner = body
                .split(" #")
                .next()
                .unwrap_or_default()
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .trim_matches(['"', '\''])
                .to_string();
            stack.clear();
            section = Some(inner);
            continue;
        }
        let parent_key = |stack: &Vec<Frame>, section: &Option<String>| {
            stack
                .last()
                .map(|f| f.key.clone())
                .or_else(|| section.clone())
        };
        let mut rest = body;
        let mut indent_of_key = indent;
        if let Some(item) = rest.strip_prefix("- ").or((rest == "-").then_some("")) {
            if keyed_item(item) {
                rest = item.trim_start();
                indent_of_key = indent + 2;
            } else {
                // an item of the list its key holds
                let value = unquote(item);
                let verdict = match parent_key(&stack, &section) {
                    Some(owner) => {
                        let above = stack.len().checked_sub(2).and_then(|n| stack.get(n));
                        judge(ctx, above.map(|f| f.key.as_str()), &owner, &value)
                    }
                    None => credential_value(&value).then(|| "item".to_string()),
                };
                if verdict.is_some() {
                    return verdict;
                }
                continue;
            }
        }
        // a line in a list opened by `[` or `<`: an item of its key's
        if stack
            .last()
            .is_some_and(|f| matches!(f.closed_by, Some(']') | Some('>')))
        {
            let owner = stack.last().map(|f| f.key.clone()).unwrap_or_default();
            let above = stack
                .len()
                .checked_sub(2)
                .and_then(|n| stack.get(n))
                .map(|f| f.key.clone());
            let inline = format!("[{}]", body.trim_end_matches(','));
            if let Some(found) = scan_json(ctx, &inline, Some(&owner)) {
                return Some(found);
            }
            if let Some(found) = judge(ctx, above.as_deref(), &owner, &unquote(body)) {
                return Some(found);
            }
            continue;
        }
        let rest = rest.strip_prefix("export ").unwrap_or(rest);
        // Forge's `.cfg` type letter: `S:botToken=…`, `S:"Bot Token"=…`
        let bytes = rest.as_bytes();
        let rest = if bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && !matches!(bytes[2], b' ' | b'/' | b'\t')
        {
            &rest[2..]
        } else {
            rest
        };
        // HOCON's and `.cfg`'s `name {`
        if let Some(name) = rest.strip_suffix('{') {
            let name = name.trim().trim_end_matches([':', '=']).trim();
            if !name.is_empty() && !name.contains(['=', ':']) {
                stack.push(Frame {
                    indent: indent_of_key,
                    key: name.trim_matches(['"', '\'']).to_string(),
                    closed_by: Some('}'),
                });
                continue;
            }
        }
        let parent = parent_key(&stack, &section);
        let Some((key, value)) = split_line(rest) else {
            let value = unquote(rest);
            if credential_value(&value) {
                return Some("value".to_string());
            }
            continue;
        };
        let value_text = value.trim();
        let push = |stack: &mut Vec<Frame>, closed_by: Option<char>| {
            stack.push(Frame {
                indent: indent_of_key,
                key: key.clone(),
                closed_by,
            });
        };
        match value_text.chars().next() {
            None | Some('#') => push(&mut stack, None),
            Some('{') | Some('[') => {
                let open = value_text.chars().next().unwrap_or('{');
                let close = if open == '{' { '}' } else { ']' };
                let opened = value_text.matches(open).count();
                let closed = value_text.matches(close).count();
                if let Some(found) = scan_json(ctx, value_text, Some(&key)) {
                    return Some(found);
                }
                if opened > closed {
                    push(&mut stack, Some(close));
                }
            }
            Some('<') if value_text == "<" => push(&mut stack, Some('>')),
            Some('|') | Some('>') => {
                // a block scalar: judged by its first line
                let first = lines[i..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .filter(|l| indentation(l) > indent)
                    .map(|l| l.trim().to_string())
                    .unwrap_or_default();
                if let Some(found) = judge(ctx, parent.as_deref(), &key, &first) {
                    return Some(found);
                }
                skip_indent = Some(indent);
            }
            _ if value_text.starts_with("\"\"\"") || value_text.starts_with("'''") => {
                let delimiter = &value_text[..3];
                let after = &value_text[3..];
                let inline = match after.find(delimiter) {
                    Some(end) => after[..end].to_string(),
                    None => {
                        skip_to = Some(if delimiter == "\"\"\"" {
                            "\"\"\""
                        } else {
                            "'''"
                        });
                        let mut first = after.trim().to_string();
                        if first.is_empty() {
                            first = lines
                                .get(i)
                                .map(|l| l.trim().to_string())
                                .unwrap_or_default();
                        }
                        first
                    }
                };
                if let Some(found) = judge(ctx, parent.as_deref(), &key, &inline) {
                    return Some(found);
                }
            }
            _ => {
                if let Some(found) = judge(ctx, parent.as_deref(), &key, &unquote(value_text)) {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up value in a Discord bot token's shape, built at run time so the source
    /// holds no token-shaped text (secret scanners flag one).
    fn fake_discord_token() -> String {
        let id = format!("M{}", "a".repeat(25));
        let time = "b".repeat(6);
        let hmac = "c".repeat(30);
        [id, time, hmac].join(".")
    }

    fn path(p: &str) -> CanonicalPath {
        CanonicalPath::new(p).unwrap()
    }

    #[test]
    fn keys_are_split_into_words() {
        assert_eq!(words("botToken"), ["bot", "token"]);
        assert_eq!(words("secret_key"), ["secret", "key"]);
        assert_eq!(words("apiKey"), ["api", "key"]);
        assert_eq!(words("rcon.password"), ["rcon", "password"]);
        assert_eq!(words("APIKey"), ["api", "key"]);
        assert_eq!(words("Bot Token"), ["bot", "token"]);
        assert_eq!(words("password2"), ["password", "2"]);
        assert_eq!(words("DISCORD_BOT_TOKEN"), ["discord", "bot", "token"]);
    }

    #[test]
    fn keys_that_name_a_credential() {
        for key in [
            "password",
            "botToken",
            "BotToken",
            "bottoken",
            "secret",
            "secret-key",
            "secretKey",
            "apiKey",
            "api_key",
            "apikey",
            "rcon.password",
            "client_secret",
            "auth-token",
            "access_token",
            "privateKey",
            "license-key",
            "webhook",
            "webhookUrl",
            "webhook-url",
            "discordWebhook",
            "credentials",
            "bearer",
            "db_pass",
            "mysql-pass",
            "dbpass",
            "passphrase",
            "sentry-dsn",
            "server-key",
            "allowed-tokens",
            "forwardingSecret",
            "Bot Token",
        ] {
            assert!(credential_key(None, key), "{key}");
        }
        assert!(credential_key(Some("secret"), "key"));
        assert!(credential_key(Some("api"), "key"));
        assert!(credential_key(Some("mysql"), "pass"));
        assert!(credential_key(Some("botToken"), "value"));
        for key in [
            "key",
            "pass",
            "keyFileName",
            "key-file-name",
            "keepInventory",
            "passive",
            "tokenExpiry",
            "token-expiry",
            "secretRooms",
            "forwarding-secret-file",
            "passwordMinLength",
            "auth-type",
            "permissionHandler",
            "username",
            "user",
            "keys",
            "hotkey",
            "monkey",
            "value",
        ] {
            assert!(!credential_key(None, key), "{key}");
        }
        assert!(!credential_key(Some("database"), "key"));
        assert!(!credential_key(Some("game"), "pass"));
    }

    #[test]
    fn placeholders_are_not_credentials() {
        for value in [
            "",
            "  ",
            "changeme",
            "your-token-here",
            "YOUR_TOKEN_HERE",
            "<token>",
            "TOKEN",
            "none",
            "null",
            "false",
            "xxxxxxxx",
            "********",
            "${BOT_TOKEN}",
            "INSERT-TOKEN-HERE",
            "INSERT BOT TOKEN HERE",
            "Paste your key",
            "BOT_TOKEN",
            "64",
            "password",
            "[]",
        ] {
            assert!(!real_value("botToken", value), "{value:?}");
        }
        let token = fake_discord_token();
        for value in [token.as_str(), "hunter2", "s3cr3t", "12345678901", "test"] {
            assert!(real_value("botToken", value), "{value:?}");
        }
    }

    #[test]
    fn values_that_are_logins_by_their_shape() {
        for value in [
            "mongodb://user:hunter2@db.example:27017",
            "jdbc:mysql://db.example/x?user=a&password=hunter2",
            "https://discord.com/api/webhooks/123456789012345678/abcdefghijklmnopqrstuvwxyz",
            fake_discord_token().as_str(),
        ] {
            assert!(credential_value(value), "{value}");
        }
        for value in [
            "mongodb://username:password@localhost:27017",
            "mongodb://localhost:27017",
            "https://spark.lucko.me/",
            "https://key@sentry.example/1",
            "jdbc:mysql://db.example/x?user=a&password=",
            "https://discord.com/api/webhooks/",
            "hello.world.again",
        ] {
            assert!(!credential_value(value), "{value}");
        }
    }

    #[test]
    fn credentials_are_found_in_every_format() {
        let json = Format::Json;
        let lines = Format::Lines;
        for (text, format) in [
            (r#"{"generic":{"botToken":"abc-real-token-1"}}"#, json),
            ("{\n  // comment\n  token: 'abc-real-token-1',\n}\n", json),
            (r#"{"secret":{"key":"abc-real-token-1"}}"#, json),
            (r#"{"allowed-tokens":["abc-real-token-1"]}"#, json),
            ("{auth:{token:\"abc-real-token-1\"}}", json),
            ("discord:\n  bot:\n    token: \"abc-real-token-1\"\n", lines),
            ("server:\n  # Secret Key\n  secret-key: 'abc-real-token-1'\n", lines),
            ("token = \"abc-real-token-1\"\n", lines),
            ("[secret]\nkey = \"abc-real-token-1\"\n", lines),
            ("[discord]\nbot-token = \"abc-real-token-1\"\n", lines),
            ("discord.bot.token = abc-real-token-1\n", lines),
            ("sponge {\n  ip-forwarding {\n    mode=MODERN\n    secret=\"abc-real-token-1\"\n  }\n}\n", lines),
            ("general {\n    S:botToken=abc-real-token-1\n}\n", lines),
            ("export BOT_TOKEN=abc-real-token-1\n", lines),
            ("rcon.password hunter22\n", lines),
            ("tokens:\n  - abc-real-token-1\n", lines),
            ("discord: {token: abc-real-token-1}\n", lines),
            ("database:\n  uri: mongodb://admin:hunter2@db:27017\n", lines),
            ("password: |\n  hunter2-on-its-own-line\n", lines),
            ("auth:\n  token: >-\n    hunter2-folded\n", lines),
            ("token = \"\"\"\nabc-real-token-1\n\"\"\"\n", lines),
            ("allowed = [\n  \"x\",\n]\ntokens = [\n  \"abc-real-token-1\",\n]\n", lines),
            ("- name: a\n  password: hunter22\n", lines),
        ] {
            assert!(find_credential(text, format).is_some(), "{text}");
        }
        for (text, format) in [
            (r#"{"botToken":"","useWebhook":true}"#, json),
            (r#"{"version":2,"continueOnRestart":false,"silent":false}"#, json),
            ("token: \"\"\nkey-file-name: key.pem\n", lines),
            ("secret = \"\"\n", lines),
            ("BotToken: \"\"\nDiscordConsoleChannelId: \"000000000000000000\"\n", lines),
            ("password: |\n\nnext: 1\n", lines),
            ("[server]\nremoveErroringBlockEntities = false\npermissionHandler = \"forge:default_handler\"\n", lines),
            ("message: |\n  enter your password: hunter2\nother: 1\n", lines),
            ("token-expiry: 3600\nmax-tokens: 64\n", lines),
        ] {
            assert_eq!(find_credential(text, format), None, "{text}");
        }
    }

    #[test]
    fn which_files_are_scanned_or_withheld_by_name() {
        for scanned in [
            "config/mc2discord.toml",
            "config/a/b.json5",
            "plugins/X/config.yml",
            "mods/tebex/config.yml",
            "defaultconfigs/x.cfg",
            "saves/world/serverconfig/ftbranks/ranks.snbt",
            "dynmap/configuration.txt",
            "usercache.json",
            "config/plasmovoice/server/forwarding-secret",
        ] {
            assert!(scanned_format(&path(scanned)).is_some(), "{scanned}");
        }
        for not in [
            "server.properties",
            "config/paper-global.yml",
            "saves/world/level.dat",
            "saves/world/data/x.json",
            "mods/sodium.jar",
            "README",
        ] {
            assert!(scanned_format(&path(not)).is_none(), "{not}");
        }
        for withheld in [
            "plugins/DiscordSRV/.token",
            "config/x/bot.token",
            "config/x/private.key",
            "config/x/cert.PEM",
            "config/x/store.p12",
            "config/x/store.jks",
            "config/x/store.keystore",
            "id_rsa",
            ".env",
            "config/x/.env.local",
            "config/x/credentials.json",
            "token.txt",
            "config/x/bot-token.txt",
            "config/x/secret_santa.txt",
            "config/plasmovoice/server/forwarding-secret",
        ] {
            assert!(withheld_by_name(&path(withheld)), "{withheld}");
        }
        for kept in [
            "config/x/keybinds.txt",
            "config/x/tokens.json",
            "saves/world/region/x.key",
            "config/x/readme.txt",
        ] {
            assert!(!withheld_by_name(&path(kept)), "{kept}");
        }
        assert!(too_large(&path("config/x.json"), MAX_SCANNED_FILE + 1));
        assert!(too_large(&path("config/x.bin"), MAX_SCANNED_FILE + 1));
        assert!(!too_large(
            &path("plugins/CoreProtect/database.db"),
            MAX_SCANNED_FILE + 1
        ));
        assert!(!too_large(&path("config/x.png"), MAX_SCANNED_FILE + 1));
        assert!(!too_large(&path("config/x.json"), MAX_SCANNED_FILE));
        assert!(!too_large(
            &path("saves/w/region/r.0.0.mca"),
            MAX_SCANNED_FILE + 1
        ));
    }

    /// A copy a plugin keeps of its settings (DiscordSRV's `config.yml-build.<version>.old`,
    /// `.bak`, `~`, `.disabled`) is read as the settings it was made from, and a copy of a
    /// key file is withheld as the key file is.
    #[test]
    fn copies_of_settings_are_read_as_what_they_were() {
        for (copy, format) in [
            (
                "plugins/DiscordSRV/config.yml-build.1.28.0.old",
                Format::Lines,
            ),
            (
                "plugins/DiscordSRV/messages.yml-build.1.28.0.old",
                Format::Lines,
            ),
            ("config/x/settings.json.bak", Format::Json),
            ("config/x/settings.JSON5.Backup", Format::Json),
            ("config/x/settings.toml~", Format::Lines),
            ("config/x/settings.yml.disabled", Format::Lines),
            ("config/x/settings.yml_2024-01-01", Format::Lines),
            ("config/x/settings.cfg (1)", Format::Lines),
            ("server.properties.old", Format::Lines),
        ] {
            assert_eq!(scanned_format(&path(copy)), Some(format), "{copy}");
        }
        for not in [
            "mods/sodium.jar.disabled",
            "config/x/icon.png.bak",
            "config/x/settings.json.gz",
            "saves/world/level.dat_old",
            "config/x/settings.configure.old",
        ] {
            assert_eq!(scanned_format(&path(not)), None, "{not}");
        }
        for withheld in [
            "config/x/bot.token.old",
            "plugins/DiscordSRV/.token.bak",
            "config/x/private.key~",
            "config/x/token.txt.bak",
        ] {
            assert!(withheld_by_name(&path(withheld)), "{withheld}");
        }
        assert!(!withheld_by_name(&path("config/x/keybinds.txt.bak")));
        let backup = format!(
            "BotToken: \"{}\"\nChannels: {{\"global\": \"000000000000000000\"}}\n",
            fake_discord_token()
        );
        assert_eq!(
            find_credential(&backup, Format::Lines),
            Some("BotToken".to_string())
        );
    }

    /// A bare `key` is a credential in a file that switches key authentication on
    /// (ServerTap's REST API key), and not otherwise.
    #[test]
    fn a_bare_key_with_key_authentication_on_is_a_credential() {
        let servertap =
            "port: 4567\ndebug: false\nuseKeyAuth: true\nkey: 'PROBE-SERVERTAP-KEY-9f8e'\n";
        assert_eq!(
            find_credential(servertap, Format::Lines),
            Some("key".to_string())
        );
        let json = "{\"rest\": {\"auth\": true, \"key\": \"PROBE-JSON-KEY\"}}";
        assert_eq!(
            find_credential(json, Format::Json),
            Some("rest.key".to_string())
        );
        for clean in [
            // the placeholder it ships
            "port: 4567\nuseKeyAuth: true\nkey: 'change_me'\n",
            // key authentication off, or no word of it
            "port: 4567\nuseKeyAuth: false\nkey: 'PROBE-SERVERTAP-KEY-9f8e'\n",
            "lang:\n  key: menu.title\n",
            "auth-type: online\nkey: abcdefgh\n",
        ] {
            assert_eq!(find_credential(clean, Format::Lines), None, "{clean}");
        }
    }

    /// Passwords settings files ship with (HuskHomes' `pa55w0rd`), a password that repeats
    /// the user's or the database's name (WorldGuard's `worldguard`) and a DSN with no login
    /// in it are defaults, not credentials; a real password beside them still is.
    #[test]
    fn shipped_database_passwords_are_not_credentials() {
        for value in [
            "pa55w0rd",
            "Passw0rd",
            "password123",
            "defaultpass",
            "guest",
        ] {
            assert!(!real_value("password", value), "{value:?}");
        }
        for clean in [
            "database:\n  type: SQLITE\n  credentials:\n    username: root\n    password: pa55w0rd\n",
            "sql:\n  use: false\n  dsn: jdbc:mysql://localhost/worldguard\n  username: worldguard\n  password: worldguard\n",
            "[db]\ndb-name = \"homes\"\npass = \"homes\"\ndb_pass = \"homes\"\n",
        ] {
            assert_eq!(find_credential(clean, Format::Lines), None, "{clean}");
        }
        for (text, key) in [
            (
                "sql:\n  username: worldguard\n  password: hunter22\n",
                "sql.password",
            ),
            (
                "sentry-dsn: https://abc123def456@o1.ingest.sentry.io/5\n",
                "sentry-dsn",
            ),
            (
                "dsn: jdbc:mysql://localhost/db?user=mc&password=hunter22\n",
                "dsn",
            ),
        ] {
            assert_eq!(
                find_credential(text, Format::Lines),
                Some(key.to_string()),
                "{text}"
            );
        }
    }
}
