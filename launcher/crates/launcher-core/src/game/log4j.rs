//! The game's console output, as structured lines.
//!
//! With Mojang's logging config, the game writes each log event to stdout as a
//! `log4j:Event` XML element (Log4j's `LegacyXMLLayout`):
//!
//! ```xml
//! <log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1700000000000" level="INFO" thread="Render thread">
//!   <log4j:Message><![CDATA[Setting user: Steve]]></log4j:Message>
//! </log4j:Event>
//! ```
//!
//! [`LogParser`] reads them line by line, with a small hand parser (the launcher has
//! no XML crate). Anything else, such as output before Log4j starts, the JVM's own
//! messages, or old versions without Log4j, becomes a plain line.

use std::fmt;

/// A log level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Level {
    pub const ALL: [Level; 6] = [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Warn,
        Level::Error,
        Level::Fatal,
    ];

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "TRACE" => Level::Trace,
            "DEBUG" => Level::Debug,
            "INFO" => Level::Info,
            "WARN" | "WARNING" => Level::Warn,
            "ERROR" | "SEVERE" => Level::Error,
            "FATAL" => Level::Fatal,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
            Level::Fatal => "FATAL",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Which pipe a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
    /// Written by the launcher itself, such as "Starting Java".
    Launcher,
}

/// One event of the game's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub level: Level,
    /// Milliseconds since the Unix epoch, from the event.
    pub timestamp: Option<u64>,
    pub logger: Option<String>,
    pub thread: Option<String>,
    pub message: String,
    /// A stack trace, when the event has one.
    pub throwable: Option<String>,
    pub stream: Stream,
}

impl LogLine {
    pub fn plain(stream: Stream, text: &str) -> Self {
        Self {
            level: guess_level(stream, text),
            timestamp: None,
            logger: None,
            thread: None,
            message: text.to_string(),
            throwable: None,
            stream,
        }
    }

    /// The line as text for "Copy log": the message and its stack trace.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        if let Some(thread) = &self.thread {
            text.push_str(&format!("[{thread}/{}] ", self.level));
        } else if self.timestamp.is_some() {
            text.push_str(&format!("[{}] ", self.level));
        }
        text.push_str(&self.message);
        if let Some(throwable) = &self.throwable {
            text.push('\n');
            text.push_str(throwable.trim_end());
        }
        text
    }
}

/// The level of a plain line: from a `[12:00:00] [main/WARN]:` prefix if it has
/// one, otherwise INFO for stdout and WARN for stderr.
fn guess_level(stream: Stream, text: &str) -> Level {
    for level in Level::ALL {
        if text.contains(&format!("/{}]", level.label()))
            || text.contains(&format!("[{}]", level.label()))
        {
            return level;
        }
    }
    if text.starts_with("Exception in thread") || text.contains("FATAL ERROR") {
        return Level::Error;
    }
    match stream {
        Stream::Stderr => Level::Warn,
        Stream::Stdout | Stream::Launcher => Level::Info,
    }
}

/// An event longer than this is given up on and shown as plain lines.
const MAX_EVENT_BYTES: usize = 1024 * 1024;

/// Turns lines of output into [`LogLine`]s. Keep one per stream.
#[derive(Debug)]
pub struct LogParser {
    stream: Stream,
    pending: Option<String>,
}

impl LogParser {
    pub fn new(stream: Stream) -> Self {
        Self {
            stream,
            pending: None,
        }
    }

    /// Feeds one line (without its line ending) and returns the finished lines.
    pub fn push(&mut self, line: &str) -> Vec<LogLine> {
        let mut out = Vec::new();
        match &mut self.pending {
            Some(pending) => {
                pending.push('\n');
                pending.push_str(line);
                if line.contains("</log4j:Event>") {
                    let event = self.pending.take().expect("pending");
                    out.extend(self.finish_event(&event));
                } else if pending.len() > MAX_EVENT_BYTES {
                    let event = self.pending.take().expect("pending");
                    out.extend(event.lines().map(|l| LogLine::plain(self.stream, l)));
                }
            }
            None => {
                if line.trim_start().starts_with("<log4j:Event") {
                    if line.contains("</log4j:Event>") || line.trim_end().ends_with("/>") {
                        out.extend(self.finish_event(line));
                    } else {
                        self.pending = Some(line.to_string());
                    }
                } else if !line.trim().is_empty() {
                    out.push(LogLine::plain(self.stream, line));
                }
            }
        }
        out
    }

    /// Flushes an unfinished event as plain lines, at the end of the output.
    pub fn finish(&mut self) -> Vec<LogLine> {
        self.pending
            .take()
            .map(|event| {
                event
                    .lines()
                    .map(|line| LogLine::plain(self.stream, line))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn finish_event(&self, xml: &str) -> Vec<LogLine> {
        match parse_event(xml, self.stream) {
            Some(line) => vec![line],
            None => xml
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| LogLine::plain(self.stream, line))
                .collect(),
        }
    }
}

/// Parses one complete `<log4j:Event ...>...</log4j:Event>` element.
pub fn parse_event(xml: &str, stream: Stream) -> Option<LogLine> {
    let start = xml.find("<log4j:Event")?;
    let rest = &xml[start + "<log4j:Event".len()..];
    let tag_end = find_tag_end(rest)?;
    let attributes = parse_attributes(&rest[..tag_end]);
    let attr = |name: &str| {
        attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let body = &rest[tag_end..];
    let message = element_text(body, "log4j:Message").unwrap_or_default();
    let throwable = element_text(body, "log4j:Throwable").filter(|t| !t.trim().is_empty());
    Some(LogLine {
        level: attr("level")
            .and_then(|level| Level::parse(&level))
            .unwrap_or(Level::Info),
        timestamp: attr("timestamp").and_then(|t| t.parse().ok()),
        logger: attr("logger"),
        thread: attr("thread"),
        message: message.trim_end_matches(['\r', '\n']).to_string(),
        throwable,
        stream,
    })
}

/// The index of the `>` that ends a start tag, skipping `>` inside quoted values.
fn find_tag_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (index, c) in text.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn parse_attributes(text: &str) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    let mut rest = text;
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let value_start = &after[1..];
        let Some(end) = value_start.find(quote) else {
            break;
        };
        attributes.push((name, unescape(&value_start[..end])));
        rest = &value_start[end + 1..];
    }
    attributes
}

/// The text of the first `<name>...</name>` in `xml`: its CDATA sections and
/// unescaped text, joined.
fn element_text(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let mut text = String::new();
    let mut rest = &xml[start..];
    loop {
        let cdata = rest.find("<![CDATA[");
        let end = rest.find(&close);
        match (cdata, end) {
            (Some(c), Some(e)) if c < e => {
                text.push_str(&unescape(&rest[..c]));
                let after = &rest[c + "<![CDATA[".len()..];
                let cdata_end = after.find("]]>")?;
                text.push_str(&after[..cdata_end]);
                rest = &after[cdata_end + 3..];
            }
            (_, Some(e)) => {
                text.push_str(&unescape(&rest[..e]));
                return Some(text);
            }
            (_, None) => return None,
        }
    }
}

/// Replaces XML's predefined entities and numeric character references.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp..];
        let Some(semi) = after.find(';').filter(|semi| *semi <= 10) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..semi];
        let decoded = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A log timestamp (milliseconds since the Unix epoch) as local `HH:MM:SS`.
pub fn format_time(timestamp_ms: u64) -> String {
    use chrono::TimeZone as _;
    match chrono::Local
        .timestamp_millis_opt(timestamp_ms as i64)
        .single()
    {
        Some(time) => time.format("%H:%M:%S").to_string(),
        None => {
            let day = timestamp_ms / 1000 % 86_400;
            format!("{:02}:{:02}:{:02}", day / 3600, day / 60 % 60, day % 60)
        }
    }
}

/// Replaces each secret in a line with `<redacted>`. Old versions print the session
/// token, so every line from the game passes through this before it is shown.
#[derive(Clone, Debug, Default)]
pub struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    pub fn new(secrets: impl IntoIterator<Item = String>) -> Self {
        Self {
            // Short values would redact ordinary words.
            secrets: secrets.into_iter().filter(|s| s.len() >= 8).collect(),
        }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut text = text.to_string();
        for secret in &self.secrets {
            if text.contains(secret.as_str()) {
                text = text.replace(secret.as_str(), "<redacted>");
            }
        }
        text
    }

    pub fn redact_line(&self, mut line: LogLine) -> LogLine {
        line.message = self.redact(&line.message);
        line.throwable = line.throwable.map(|t| self.redact(&t));
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(text: &str) -> Vec<LogLine> {
        let mut parser = LogParser::new(Stream::Stdout);
        let mut lines: Vec<LogLine> = text.lines().flat_map(|line| parser.push(line)).collect();
        lines.extend(parser.finish());
        lines
    }

    #[test]
    fn parses_events() {
        let output = r#"<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1700000000000" level="INFO" thread="Render thread">
  <log4j:Message><![CDATA[Setting user: Steve]]></log4j:Message>
</log4j:Event>
<log4j:Event logger="dcz" timestamp="1700000000123" level="WARN" thread="Worker-Main-1">
  <log4j:Message><![CDATA[Missing sound for event: minecraft:item.goat_horn.sound.0]]></log4j:Message>
</log4j:Event>"#;
        let lines = parse_all(output);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            LogLine {
                level: Level::Info,
                timestamp: Some(1_700_000_000_000),
                logger: Some("net.minecraft.client.Minecraft".into()),
                thread: Some("Render thread".into()),
                message: "Setting user: Steve".into(),
                throwable: None,
                stream: Stream::Stdout,
            }
        );
        assert_eq!(lines[1].level, Level::Warn);
        assert_eq!(lines[1].thread.as_deref(), Some("Worker-Main-1"));
    }

    #[test]
    fn parses_throwables_multiline_messages_and_entities() {
        let output = "<log4j:Event logger=\"a &amp; b\" timestamp=\"1\" level=\"ERROR\" thread=\"main\">\n  <log4j:Message><![CDATA[Failed to load\nsecond line]]></log4j:Message>\n  <log4j:Throwable><![CDATA[java.lang.IllegalStateException: boom\n\tat a.b.C.d(C.java:1)\n]]></log4j:Throwable>\n</log4j:Event>";
        let lines = parse_all(output);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.logger.as_deref(), Some("a & b"));
        assert_eq!(line.message, "Failed to load\nsecond line");
        assert!(
            line.throwable
                .as_deref()
                .unwrap()
                .starts_with("java.lang.IllegalStateException: boom\n\tat a.b.C.d")
        );
        assert_eq!(line.level, Level::Error);
    }

    #[test]
    fn text_without_cdata_is_unescaped() {
        let line = parse_event(
            "<log4j:Event level=\"DEBUG\" thread='t'><log4j:Message>a &lt;b&gt; &#65;&#x42; &bogus; & c</log4j:Message></log4j:Event>",
            Stream::Stdout,
        )
        .unwrap();
        assert_eq!(line.message, "a <b> AB &bogus; & c");
        assert_eq!(line.thread.as_deref(), Some("t"));
        assert_eq!(line.level, Level::Debug);
    }

    #[test]
    fn split_cdata_sections_join() {
        let line = parse_event(
            "<log4j:Event level=\"INFO\"><log4j:Message><![CDATA[a ]]]]><![CDATA[> b]]></log4j:Message></log4j:Event>",
            Stream::Stdout,
        )
        .unwrap();
        assert_eq!(line.message, "a ]]> b");
    }

    #[test]
    fn falls_back_to_plain_lines() {
        let lines = parse_all(
            "Picked up JAVA_TOOL_OPTIONS: -Xss2m\n[12:00:01] [main/WARN]: Old-style log line\n\n<log4j:Event level=\"INFO\" thread=\"main\">\n<log4j:Message><![CDATA[unterminated",
        );
        assert_eq!(lines.len(), 4, "{lines:#?}");
        assert_eq!(lines[0].level, Level::Info);
        assert_eq!(lines[0].message, "Picked up JAVA_TOOL_OPTIONS: -Xss2m");
        assert_eq!(lines[1].level, Level::Warn);
        // The unfinished event is flushed as plain text at the end.
        assert!(lines[2].message.starts_with("<log4j:Event"));
        let mut stderr = LogParser::new(Stream::Stderr);
        let line = stderr.push("[LWJGL] Failed to load a library").remove(0);
        assert_eq!(line.level, Level::Warn);
        assert_eq!(
            stderr.push("Exception in thread \"main\" java.lang.UnsatisfiedLinkError: x")[0].level,
            Level::Error
        );
    }

    #[test]
    fn unknown_levels_are_info_and_bad_xml_is_plain() {
        let line = parse_event(
            "<log4j:Event level=\"LOUD\"><log4j:Message>x</log4j:Message></log4j:Event>",
            Stream::Stdout,
        )
        .unwrap();
        assert_eq!(line.level, Level::Info);
        let lines = parse_all("<log4j:Event level=\"INFO\" </log4j:Event>");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn copies_as_text() {
        let line = LogLine {
            level: Level::Error,
            timestamp: Some(1),
            logger: None,
            thread: Some("Render thread".into()),
            message: "Crash".into(),
            throwable: Some("java.lang.Error\n\tat x\n".into()),
            stream: Stream::Stdout,
        };
        assert_eq!(
            line.to_text(),
            "[Render thread/ERROR] Crash\njava.lang.Error\n\tat x"
        );
    }

    #[test]
    fn formats_times() {
        let time = format_time(1_700_000_000_000);
        assert_eq!(time.len(), 8);
        assert_eq!(time.matches(':').count(), 2);
        assert!(
            time.chars()
                .filter(|c| *c != ':')
                .all(|c| c.is_ascii_digit())
        );
    }

    #[test]
    fn redacts_secrets() {
        let redactor = Redactor::new([
            "eyJhbGciOiJIUzI1NiJ9.token".to_string(),
            "short".to_string(),
        ]);
        assert_eq!(
            redactor.redact("(Session ID is token:eyJhbGciOiJIUzI1NiJ9.token:uuid) short"),
            "(Session ID is token:<redacted>:uuid) short"
        );
        let line = redactor.redact_line(LogLine::plain(
            Stream::Stdout,
            "x eyJhbGciOiJIUzI1NiJ9.token",
        ));
        assert_eq!(line.message, "x <redacted>");
    }
}
