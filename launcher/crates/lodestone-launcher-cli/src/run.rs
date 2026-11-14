//! Running a command: connecting, calling, waiting, following, printing.

use std::io::{BufRead as _, IsTerminal as _, Write as _};

use clap::Parser as _;
use launcher_control::{ErrorObject, codes};
use serde_json::json;

use crate::{
    cli::{Cli, Global},
    output::{self, Mode},
    plan::{self, Plan},
};

/// Parses the command line, runs it, and returns the exit code.
pub fn main() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return usage_error(err),
    };
    let mode = mode(&cli.global, std::io::stdout().is_terminal());
    match run(&cli, mode) {
        Ok(()) => 0,
        Err(error) => {
            output::error(mode, &error);
            error.exit_code()
        }
    }
}

/// JSON unless asked for text, or on a terminal without --json.
pub fn mode(global: &Global, stdout_is_terminal: bool) -> Mode {
    if global.json || (!global.human && !stdout_is_terminal) {
        Mode::Json
    } else {
        Mode::Human
    }
}

fn usage_error(err: clap::Error) -> i32 {
    use clap::error::ErrorKind;
    if matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        let _ = err.print();
        return 0;
    }
    let args: Vec<String> = std::env::args().collect();
    let json = args.iter().any(|a| a == "--json")
        || (!args.iter().any(|a| a == "--human") && !std::io::stdout().is_terminal());
    if json {
        let error = ErrorObject::new(codes::USAGE, err.render().to_string().trim().to_string())
            .with_data(json!({ "help": "Run with --help, or `lodestone-launcher-cli schema` for every command as JSON." }));
        output::error(Mode::Json, &error);
    } else {
        let _ = err.print();
    }
    2
}

fn run(cli: &Cli, mode: Mode) -> Result<(), ErrorObject> {
    let mut read_line = || {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        Ok(line)
    };
    let plan = plan::plan(&cli.command, &mut read_line)?;
    if plan == Plan::Schema {
        let schema = crate::schema::schema();
        match mode {
            Mode::Json => output::result(Mode::Json, &schema),
            Mode::Human => println!(
                "{}",
                serde_json::to_string_pretty(&schema).unwrap_or_default()
            ),
        }
        return Ok(());
    }
    connected::run(plan, &cli.global, mode)
}

/// Asks on the terminal; only when both stdin and stderr are one.
/// Whether the user is at a terminal to answer.
pub(crate) fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

fn ask(question: &str) -> bool {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return false;
    }
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

mod connected {
    use std::time::{Duration, Instant};

    use launcher_control::{
        ErrorObject, Message,
        client::{Client, ClientError},
        codes, paths,
    };
    use serde_json::{Value, json};

    use super::ask;
    use crate::{
        cli::Global,
        output::{self, Mode},
        plan::{Follow, Plan},
    };

    pub fn run(plan: Plan, global: &Global, mode: Mode) -> Result<(), ErrorObject> {
        let mut client = connect(global)?;
        match plan {
            Plan::Schema => Ok(()),
            Plan::Call {
                method,
                params,
                long,
            } => call(&mut client, global, mode, method, params, long),
            Plan::Logs {
                instance,
                lines,
                follow,
            } => logs(&mut client, global, mode, &instance, lines, follow),
            Plan::Events { params, count } => events(&mut client, global, mode, params, count),
            Plan::Trace { params, count } => trace(&mut client, global, mode, params, count),
            Plan::PerfWatch { count } => perf_watch(&mut client, global, mode, count),
        }
    }

    fn connect(global: &Global) -> Result<Client, ErrorObject> {
        let path = global
            .socket
            .clone()
            .or_else(paths::default_socket)
            .ok_or_else(|| {
                ErrorObject::new(
                    codes::NOT_RUNNING,
                    "No home folder, so the launcher's data directory is unknown. Pass --socket.",
                )
            })?;
        let mut client = Client::connect(&path).map_err(|err| err.to_error())?;
        hello(&mut client);
        Ok(client)
    }

    /// Says which program this is, for the launcher's Settings > Developer list. A
    /// launcher too old to know `meta.hello` answers method_not_found: nothing lost.
    fn hello(client: &mut Client) {
        let _ = client.call(
            "meta.hello",
            json!({
                "client": format!("lodestone-launcher-cli {}", env!("CARGO_PKG_VERSION")),
                "pid": std::process::id(),
            }),
        );
    }

    /// Whether a stream goes on after printing a line. Once stdout is gone (its reader,
    /// such as `head`, exited) nothing more can be shown, so the stream ends without a
    /// failure, as it does with SIGPIPE in other tools. Any other write error is one.
    fn printed(result: std::io::Result<()>) -> Result<bool, ErrorObject> {
        match result {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(false),
            Err(err) => Err(ErrorObject::new(
                codes::FAILED,
                format!("Couldn't write the output: {err}"),
            )),
        }
    }

    fn deadline(global: &Global) -> Option<Instant> {
        global
            .timeout
            .map(|secs| Instant::now() + Duration::from_secs(secs))
    }

    fn left(deadline: Option<Instant>) -> Result<Option<Duration>, ErrorObject> {
        match deadline {
            None => Ok(None),
            Some(deadline) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    Err(ClientError::Timeout.to_error())
                } else {
                    Ok(Some(left))
                }
            }
        }
    }

    fn call(
        client: &mut Client,
        global: &Global,
        mode: Mode,
        method: &str,
        mut params: Value,
        long: bool,
    ) -> Result<(), ErrorObject> {
        if long {
            params["wait"] = json!(!global.no_wait);
        }
        if global.yes {
            params["confirm"] = json!(true);
        }
        let timeout = global.timeout.map(Duration::from_secs);
        let mut asked = false;
        loop {
            let mut last = String::new();
            let result = client
                .call_with(
                    method,
                    params.clone(),
                    timeout,
                    |progress| {
                        let text = match mode {
                            Mode::Json => progress.to_string(),
                            Mode::Human => output::progress_text(progress),
                        };
                        if text != last {
                            output::progress(mode, progress);
                            last = text;
                        }
                    },
                    |_| {},
                )
                .map_err(|err| err.to_error())?;
            match result {
                Ok(value) => {
                    output::result(mode, &value);
                    return Ok(());
                }
                Err(error)
                    if error.code == codes::CONFIRMATION_REQUIRED && !global.yes && !asked =>
                {
                    // on a terminal, ask; otherwise fail and say to pass --yes
                    if !super::interactive() {
                        return Err(error);
                    }
                    output::error(Mode::Human, &error);
                    if !ask("Go ahead?") {
                        return Err(error);
                    }
                    asked = true;
                    params["confirm"] = json!(true);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn logs(
        client: &mut Client,
        global: &Global,
        mode: Mode,
        instance: &str,
        lines: usize,
        follow: Option<Follow>,
    ) -> Result<(), ErrorObject> {
        let value = client
            .call("play.logs", json!({ "instance": instance, "lines": lines }))
            .map_err(|err| err.to_error())??;
        let rows = value["rows"].as_array().cloned().unwrap_or_default();
        let Some(follow) = follow else {
            match mode {
                Mode::Json => output::result(mode, &value),
                Mode::Human => {
                    for row in &rows {
                        if !printed(output::line(mode, row))? {
                            break;
                        }
                    }
                }
            }
            return Ok(());
        };
        for row in &rows {
            if !printed(output::line(mode, row))? {
                return Ok(());
            }
        }
        if follow.until_exit && value["state"] == "idle" {
            return Ok(());
        }
        let id = value["instance"].as_str().unwrap_or(instance).to_string();
        // from where play.logs left off, so no line is lost between the two requests,
        // and with the state it said, so a game that exited in between still ends this
        let subscribed = client
            .call(
                "events.subscribe",
                json!({
                    "topics": ["log", "state"],
                    "instance": id,
                    "since": value["cursor"],
                    "last_state": value["state"],
                }),
            )
            .map_err(|err| err.to_error())??;
        let subscription = subscribed["subscription"].as_u64();
        let deadline = deadline(global);
        loop {
            let message = client
                .next_message(left(deadline)?)
                .map_err(|err| err.to_error())?;
            let Message::Event {
                subscription: number,
                event,
                data,
            } = message
            else {
                continue;
            };
            if Some(number) != subscription {
                continue;
            }
            match event.as_str() {
                "log" => {
                    if !printed(output::line(mode, &data))? {
                        return Ok(());
                    }
                    if let Some(until) = &follow.until
                        && data["text"]
                            .as_str()
                            .is_some_and(|t| t.contains(until.as_str()))
                    {
                        return Ok(());
                    }
                }
                "state" if follow.until_exit && data["state"] == "idle" => return Ok(()),
                "lagged" => output::progress(
                    mode,
                    &json!({ "label": "Some lines were dropped: the terminal read too slowly.", "dropped": data["dropped"] }),
                ),
                _ => {}
            }
        }
    }

    /// `trace watch`: each event as a line, `HH:MM:SS.mmm topic {data}` for people.
    fn trace(
        client: &mut Client,
        global: &Global,
        mode: Mode,
        params: Value,
        count: Option<usize>,
    ) -> Result<(), ErrorObject> {
        let subscribed = client
            .call("trace.subscribe", params)
            .map_err(|err| err.to_error())??;
        let subscription = subscribed["subscription"].as_u64();
        let deadline = deadline(global);
        let mut seen = 0;
        while count.is_none_or(|count| seen < count) {
            let wait = match deadline {
                None => None,
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(());
                    }
                    Some(left)
                }
            };
            let message = match client.next_message(wait) {
                Ok(message) => message,
                Err(ClientError::Timeout) => return Ok(()),
                Err(err) => return Err(err.to_error()),
            };
            if let Message::Event {
                subscription: number,
                event,
                data,
            } = message
                && Some(number) == subscription
                && event == "trace"
            {
                let line = match mode {
                    Mode::Json => output::line(mode, &data),
                    Mode::Human => output::text_line(&output::trace_line(&data)),
                };
                if !printed(line)? {
                    return Ok(());
                }
                seen += 1;
            }
        }
        Ok(())
    }

    fn perf_watch(
        client: &mut Client,
        global: &Global,
        mode: Mode,
        count: Option<usize>,
    ) -> Result<(), ErrorObject> {
        let subscribed = client
            .call("perf.watch", json!({}))
            .map_err(|err| err.to_error())??;
        let subscription = subscribed["subscription"].as_u64();
        let deadline = deadline(global);
        let mut seen = 0;
        while count.is_none_or(|count| seen < count) {
            let wait = match deadline {
                None => None,
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(());
                    }
                    Some(left)
                }
            };
            let message = match client.next_message(wait) {
                Ok(message) => message,
                Err(ClientError::Timeout) => return Ok(()),
                Err(err) => return Err(err.to_error()),
            };
            if let Message::Event {
                subscription: number,
                event,
                data,
            } = message
                && Some(number) == subscription
                && event == "perf"
            {
                if !printed(output::line(mode, &data))? {
                    return Ok(());
                }
                seen += 1;
            }
        }
        Ok(())
    }

    fn events(
        client: &mut Client,
        global: &Global,
        mode: Mode,
        params: Value,
        count: Option<usize>,
    ) -> Result<(), ErrorObject> {
        let subscribed = client
            .call("events.subscribe", params)
            .map_err(|err| err.to_error())??;
        let subscription = subscribed["subscription"].as_u64();
        let deadline = deadline(global);
        let mut seen = 0;
        while count.is_none_or(|count| seen < count) {
            // a deadline for a stream ends it; it isn't a failure
            let wait = match deadline {
                None => None,
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(());
                    }
                    Some(left)
                }
            };
            let message = match client.next_message(wait) {
                Ok(message) => message,
                Err(ClientError::Timeout) => return Ok(()),
                Err(err) => return Err(err.to_error()),
            };
            if let Message::Event {
                subscription: number,
                event,
                data,
            } = message
                && Some(number) == subscription
            {
                if !printed(output::line(mode, &json!({ "event": event, "data": data })))? {
                    return Ok(());
                }
                seen += 1;
            }
        }
        Ok(())
    }
}
