mod app;
mod config;
mod herdr;
mod jira;
mod mock;
mod open_issue;
mod resource;
mod ui;

use app::{App, Resp};
use crossterm::event::{self, Event, KeyEventKind};
use std::sync::mpsc;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return dispatch_cli(&args[1..]);
    }
    run_tui()
}

fn dispatch_cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("resource") => {
            let mut resource_id = "my-issues";
            let mut force_mock = mock::mock_enabled();
            let mut index = 1usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--id" => {
                        index += 1;
                        resource_id = args
                            .get(index)
                            .map(String::as_str)
                            .unwrap_or(resource_id);
                    }
                    "--mock" => force_mock = true,
                    "--help" | "-h" => {
                        eprintln!("usage: herdr-jira resource --id my-issues [--mock]");
                        return Ok(());
                    }
                    other => {
                        eprintln!("unknown resource argument: {other}");
                        std::process::exit(2);
                    }
                }
                index += 1;
            }
            if let Err(error) = resource::run(resource_id, force_mock) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            Ok(())
        }
        Some("open-issue") => {
            if let Err(error) = open_issue::run() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            Ok(())
        }
        Some("--open") => {
            let key = args.get(1).cloned().unwrap_or_default();
            if key.is_empty() {
                eprintln!("usage: herdr-jira --open ISSUE-KEY");
                std::process::exit(2);
            }
            std::env::set_var("HERDR_JIRA_OPEN_ISSUE", &key);
            run_tui()
        }
        Some("--help") | Some("-h") => {
            eprintln!("herdr-jira");
            eprintln!("  herdr-jira");
            eprintln!("  herdr-jira --open ISSUE-KEY");
            eprintln!("  herdr-jira resource --id my-issues [--mock]");
            eprintln!("  herdr-jira open-issue");
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
        None => run_tui(),
    }
}

fn run_tui() -> std::io::Result<()> {
    let (tx, rx) = mpsc::channel::<Resp>();
    let mut app = App::new(tx);
    app.apply_startup_open_issue();

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, rx);
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: mpsc::Receiver<Resp>,
) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        while let Ok(resp) = rx.try_recv() {
            app.on_resp(resp);
        }
        app.poll_open_issue_request();
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                _ => {}
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
