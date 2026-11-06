//! CLAUDE (claude-opus-5-5): sends js to a running app's remote console (corn_scripting's `remote_console` feature, the
//! app started with `CORN_DEV=<port>`), and prints the reply.
//!
//! ```text
//! corn-dev [options] [code]      code from the argument, a file (-f), or stdin
//!   -f FILE    the code is FILE's contents
//!   -g         run it as a generator body: it may `yield` (seconds), `yield* screenshot()`, and `return`s the value
//!   -p PORT    the app's port (default: $CORN_DEV, else 7777)
//!   -t SECS    give up waiting for the reply after SECS (default 300)
//!   --json     print the reply as it came
//! ```
//!
//! prints the value as JSON (nothing for undefined), with the request's `console.log` output first, on
//! stderr. exits 1 if the code threw (its error and stack on stderr), 2 if the app can't be reached.
//!
//! examples: `corn-dev 'find("Player")'`, `corn-dev 'screenshot()'` (prints the png's path),
//! `corn-dev -g 'component(find("Player")[0], "Player").yaw = 1; yield 0.5; return yield* screenshot("a.png")'`

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::ExitCode,
    time::Duration,
};

fn usage() -> ExitCode {
    eprintln!("usage: corn-dev [-g] [-p PORT] [-t SECS] [--json] [-f FILE | CODE]   (no code: read stdin)");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut generator, mut raw, mut timeout) = (false, false, 300.0);
    let mut port: Option<String> = None;
    let mut code: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-g" => generator = true,
            "--json" => raw = true,
            "-p" => port = args.next(),
            "-t" => match args.next().and_then(|t| t.parse().ok()) {
                Some(t) => timeout = t,
                None => return usage(),
            },
            "-f" => match args.next().map(std::fs::read_to_string) {
                Some(Ok(source)) => code = Some(source),
                Some(Err(e)) => {
                    eprintln!("corn-dev: {e}");
                    return ExitCode::from(2);
                }
                None => return usage(),
            },
            "-h" | "--help" => return usage(),
            _ if code.is_none() => code = Some(arg),
            _ => return usage(),
        }
    }
    let code = match code {
        Some(code) => code,
        None => {
            let mut source = String::new();
            if std::io::stdin().read_to_string(&mut source).is_err() {
                return usage();
            }
            source
        }
    };
    let port = port.or_else(|| std::env::var("CORN_DEV").ok()).filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "7777".into());

    let reply = match ask(&port, &code, generator, timeout) {
        Ok(reply) => reply,
        Err(e) => {
            eprintln!("corn-dev: 127.0.0.1:{}: {e} (is the app running, with CORN_DEV set?)", port.trim());
            return ExitCode::from(2);
        }
    };
    if raw {
        println!("{reply}");
    }
    let Ok(reply) = serde_json::from_str::<serde_json::Value>(&reply) else {
        eprintln!("corn-dev: not a JSON reply: {reply}");
        return ExitCode::from(2);
    };
    if raw {
        return if reply["ok"] == true { ExitCode::SUCCESS } else { ExitCode::from(1) };
    }
    for line in reply["logs"].as_array().into_iter().flatten() {
        eprintln!("{}", line.as_str().unwrap_or_default());
    }
    if reply["ok"] != true {
        eprintln!("{}", reply["error"].as_str().unwrap_or("failed"));
        return ExitCode::from(1);
    }
    match &reply["value"] {
        serde_json::Value::Null if reply.get("value").is_none() => {}
        // a plain string prints as itself (paths, ids)
        serde_json::Value::String(s) => println!("{s}"),
        value => println!("{}", serde_json::to_string_pretty(value).unwrap()),
    }
    ExitCode::SUCCESS
}

fn ask(port: &str, code: &str, generator: bool, timeout: f64) -> std::io::Result<String> {
    let stream = TcpStream::connect(("127.0.0.1", port.trim().parse::<u16>().map_err(std::io::Error::other)?))?;
    stream.set_read_timeout(Some(Duration::from_secs_f64(timeout)))?;
    let mut request = serde_json::json!({ "code": code, "generator": generator }).to_string();
    request.push('\n');
    (&stream).write_all(request.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply)?;
    if reply.is_empty() {
        return Err(std::io::Error::other("the app closed the connection"));
    }
    Ok(reply.trim_end().to_string())
}
