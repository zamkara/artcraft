use super::*;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState, Wrap},
};
use std::{
    io::{BufRead, BufReader},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

// Every style uses the terminal's default foreground and background. Selection
// only reverses those colors; no RGB palette, bold text, icons or animation.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    description: String,
    local: Option<String>,
    remote: Option<String>,
    size: Option<u64>,
    upstream: String,
    status: String,
}
enum Message {
    Log(String),
    Catalog(Vec<Entry>),
    Text(String, String),
    Confirm(String, Sender<bool>),
    Password(Sender<Option<String>>),
    Done(Result<()>),
}
#[derive(Clone)]
struct ScreenFeedback(Sender<Message>);
impl Feedback for ScreenFeedback {
    fn log(&self, text: String) {
        let _ = self.0.send(Message::Log(text));
    }
    fn confirm(&self, text: String) -> Result<bool> {
        let (tx, rx) = mpsc::channel();
        self.0
            .send(Message::Confirm(text, tx))
            .context("Interface closed")?;
        rx.recv().context("Confirmation cancelled")
    }
    fn password(&self) -> Result<Option<String>> {
        let (tx, rx) = mpsc::channel();
        self.0
            .send(Message::Password(tx))
            .context("Interface closed")?;
        rx.recv().context("Authentication cancelled")
    }
}
struct RestoreTerminal;
impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}
enum Popup {
    Text {
        title: String,
        text: String,
        scroll: u16,
    },
    Confirm {
        text: String,
        scroll: u16,
        reply: Sender<bool>,
    },
    Password {
        input: String,
        reply: Sender<Option<String>>,
    },
}
struct Screen {
    entries: Vec<Entry>,
    all_entries: Vec<Entry>,
    show_catalog: bool,
    loaded: bool,
    table: TableState,
    marked: BTreeSet<String>,
    logs: Vec<String>,
    log_scroll: u16,
    log_focus: bool,
    busy: bool,
    popup: Option<Popup>,
}
impl Screen {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            all_entries: Vec::new(),
            show_catalog: false,
            loaded: false,
            table: TableState::default(),
            marked: BTreeSet::new(),
            logs: vec!["Reading installed applications...".into()],
            log_scroll: 0,
            log_focus: false,
            busy: false,
            popup: None,
        }
    }
    fn update(&mut self, rows: Vec<Entry>) {
        self.loaded = true;
        let current = self.current().map(|e| e.name.clone());
        self.all_entries = rows;
        self.entries = self
            .all_entries
            .iter()
            .filter(|e| self.show_catalog || e.local.is_some())
            .cloned()
            .collect();
        self.marked
            .retain(|name| self.entries.iter().any(|e| e.name == *name));
        self.table.select(
            current
                .and_then(|name| self.entries.iter().position(|e| e.name == name))
                .or({
                    if self.entries.is_empty() {
                        None
                    } else {
                        Some(0)
                    }
                }),
        );
    }
    fn current(&self) -> Option<&Entry> {
        self.table.selected().and_then(|i| self.entries.get(i))
    }
    fn targets(&self) -> Vec<String> {
        if self.marked.is_empty() {
            self.current()
                .map(|e| vec![e.name.clone()])
                .unwrap_or_default()
        } else {
            self.marked.iter().cloned().collect()
        }
    }
    fn log(&mut self, text: String) {
        // Child output is data, not terminal control sequences.
        for line in text.lines() {
            self.logs.push(clean_text(line));
        }
        if self.logs.len() > 2000 {
            self.logs.drain(..self.logs.len() - 2000);
        }
        self.log_scroll = 0;
    }
    fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let i = self
            .table
            .selected()
            .unwrap_or(0)
            .saturating_add_signed(delta)
            .min(self.entries.len() - 1);
        self.table.select(Some(i));
    }
}
fn clean_text(value: &str) -> String {
    let mut result = String::new();
    let mut escape = false;
    for ch in value.chars() {
        if ch == '\u{1b}' {
            escape = true;
            continue;
        }
        if escape {
            if ch.is_ascii_alphabetic() {
                escape = false;
            }
            continue;
        }
        if !ch.is_control() || ch == '\t' {
            result.push(ch);
        }
    }
    result
}
fn entries(catalog: &BTreeMap<String, Available>) -> Result<Vec<Entry>> {
    let local_apps = catalog::installed_apps()?;
    let names: BTreeSet<String> = local_apps
        .keys()
        .chain(catalog.keys())
        .filter(|n| n.as_str() != "artcraft")
        .cloned()
        .collect();
    let mut result = Vec::new();
    for name in names {
        let local = local_apps.get(&name).map(|(version, _)| version.clone());
        let remote = catalog.get(&name);
        let status = match (&local, remote) {
            (Some(local), Some(remote)) if newer(&remote.version, local)? => "Upgrade available",
            (Some(_), Some(_)) => "Current",
            (Some(_), None) => "Installed",
            (None, Some(_)) => "Not installed",
            (None, None) => "Unknown",
        };
        let info = remote
            .map(|r| &r.info)
            .filter(|i| !i.repo.is_empty())
            .or_else(|| local_apps.get(&name).map(|(_, i)| i));
        result.push(Entry {
            name,
            description: info
                .map(|i| i.description.clone())
                .unwrap_or_else(|| "Native Storytold application".into()),
            upstream: info.map(|i| i.repo.clone()).unwrap_or_default(),
            local,
            remote: remote.map(|r| r.version.clone()),
            size: remote.map(|r| r.package.size),
            status: status.into(),
        });
    }
    Ok(result)
}
#[derive(Clone)]
enum Action {
    Load,
    Sync,
    Check,
    Install(Vec<String>),
    Upgrade(Vec<String>),
    Remove(Vec<String>),
    Changelog(String),
    Open(String),
}
fn start(action: Action, tx: Sender<Message>) {
    thread::spawn(move || {
        let feedback = ScreenFeedback(tx.clone());
        let result = (|| -> Result<()> {
            if let Action::Remove(targets) = &action {
                let mut args = vec!["-R".into(), "--".into()];
                args.extend(targets.clone());
                transaction(&args, &feedback)?;
            } else if let Action::Open(name) = &action {
                ensure!(name != "artcraft", "Artcraft is already open");
                ensure!(installed(name)?.is_some(), "{name} is not installed");
                Process::new(name)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .with_context(|| format!("Could not open {name}"))?;
                feedback.log(format!("Opened {name}"));
                return Ok(());
            } else {
                let client = client()?;

                let refresh = matches!(
                    action,
                    Action::Sync | Action::Install(_) | Action::Upgrade(_)
                );
                if refresh {
                    feedback.log("Synchronizing release catalog...".into());
                }
                let catalog = catalog::load(&client, refresh)?;
                let supported: BTreeSet<String> = catalog
                    .keys()
                    .cloned()
                    .chain(catalog::installed_apps()?.into_keys())
                    .chain(std::iter::once("artcraft".into()))
                    .collect();
                tx.send(Message::Catalog(entries(&catalog)?))?;
                match action {
                    Action::Load => feedback
                        .log("Installed apps loaded. Press S to sync the full catalog; a / l to switch lists.".into()),
                    Action::Sync | Action::Check => {
                        let rows = entries(&catalog)?;
                        for row in rows.iter().filter(|e| e.status == "Upgrade available") {
                            feedback.log(format!(
                                "{}: {} -> {}",
                                row.name,
                                row.local.as_deref().unwrap_or(""),
                                row.remote.as_deref().unwrap_or("")
                            ));
                        }
                        if !rows.iter().any(|e| e.status == "Upgrade available") {
                            feedback.log("No upgrades available in this catalog.".into());
                        }
                        if matches!(action, Action::Sync) {
                            feedback.log("Catalog synchronized. No packages were changed.".into());
                        }
                    }
                    Action::Install(targets) => {
                        install_with_feedback(&client, &catalog, &targets, false, Some(&feedback))?
                    }
                    Action::Upgrade(targets) => {
                        let targets = if targets.is_empty() {
                            supported.into_iter().collect()
                        } else {
                            targets
                        };
                        install_with_feedback(&client, &catalog, &targets, true, Some(&feedback))?;
                    }
                    Action::Changelog(name) => {
                        let remote = catalog
                            .get(&name)
                            .context("No published release for this app")?;
                        let text = if let Some(asset) = remote.release.assets.iter().find(|a| {
                            a.name == format!("{name}.CHANGELOG.md") || a.name == "CHANGELOG.md"
                        }) {
                            String::from_utf8(get(
                                &client,
                                asset_url(asset, &remote.release.tag_name)?,
                                4 * 1024 * 1024,
                            )?)?
                        } else {
                            remote
                                .release
                                .body
                                .clone()
                                .unwrap_or_else(|| "No changelog available.".into())
                        };
                        tx.send(Message::Text(format!("{name} — Upstream changelog"), text))?;
                    }
                    Action::Remove(_) | Action::Open(_) => unreachable!(),
                }
                tx.send(Message::Catalog(entries(&catalog)?))?;
                return Ok(());
            }
            let client = client()?;
            let catalog = catalog::load(&client, false)?;
            tx.send(Message::Catalog(entries(&catalog)?))?;
            Ok(())
        })();
        let _ = tx.send(Message::Done(result));
    });
}
fn short_version(version: Option<&str>) -> &str {
    let Some(version) = version else {
        return "—";
    };
    let pkgver = version.rsplit_once('-').map_or(version, |(v, _)| v);
    if let Some((base, revision)) = pkgver.rsplit_once(".r") {
        if revision.split_once(".g").is_some_and(|(date, hash)| {
            date.len() == 14
                && date.bytes().all(|b| b.is_ascii_digit())
                && !hash.is_empty()
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
        }) {
            return base;
        }
    }
    pkgver
}
fn draw(frame: &mut Frame, screen: &mut Screen) {
    let area = frame.area();
    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(7),
        Constraint::Length(7),
        Constraint::Length(3),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(if screen.busy {
            "Artcraft   Working..."
        } else {
            "Artcraft   Native Arch applications"
        }),
        sections[0],
    );
    let (table_area, detail_area) = if area.width >= 110 {
        let columns = Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(sections[1]);
        (columns[0], Some(columns[1]))
    } else {
        (sections[1], None)
    };
    let version_width = table_area.width.saturating_sub(44).saturating_div(2).max(1);
    let rows: Vec<Row> = screen
        .entries
        .iter()
        .map(|e| {
            Row::new(vec![
                if screen.marked.contains(&e.name) {
                    "[x]"
                } else {
                    "[ ]"
                }
                .to_string(),
                e.name.clone(),
                short_version(e.local.as_deref()).into(),
                short_version(e.remote.as_deref()).into(),
                e.status.clone(),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(18),
            Constraint::Length(version_width),
            Constraint::Length(version_width),
            Constraint::Length(17),
        ],
    )
    .header(Row::new([
        "",
        "Application",
        "Installed version",
        "Available version",
        "Status",
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(if screen.show_catalog {
                " All applications "
            } else {
                " Installed applications "
            }),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    if screen.entries.is_empty() {
        let message = if !screen.loaded {
            "Reading installed applications..."
        } else if screen.show_catalog {
            "The saved catalog is empty.\nPress S to synchronize available applications."
        } else {
            "No applications installed.\nPress S to synchronize the catalog and choose an app to install."
        };
        frame.render_widget(
            Paragraph::new(message).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(if screen.show_catalog {
                        " All applications "
                    } else {
                        " Installed applications "
                    }),
            ),
            table_area,
        );
    } else {
        frame.render_stateful_widget(table, table_area, &mut screen.table);
    }
    let detail = screen
        .current()
        .map(|e| {
            format!(
                "{}\n\n{}\n\nInstalled: {}\nAvailable: {}\nPackage: {}\n\nUpstream: github.com/{}",
                e.name,
                e.description,
                e.local.as_deref().unwrap_or("Not installed"),
                e.remote.as_deref().unwrap_or("Not published"),
                e.size
                    .map(|s| format!("{:.2} MiB", s as f64 / 1048576.0))
                    .unwrap_or_else(|| "Unknown".into()),
                e.upstream
            )
        })
        .unwrap_or_default();
    if let Some(detail_area) = detail_area {
        frame.render_widget(
            Paragraph::new(detail)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title(" Details ")),
            detail_area,
        );
    }
    let visible = sections[2].height.saturating_sub(2) as usize;
    let bottom = screen.logs.len().saturating_sub(visible);
    let offset = bottom.saturating_sub(screen.log_scroll as usize);
    let log = screen
        .logs
        .iter()
        .skip(offset)
        .take(visible)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(log).block(Block::default().borders(Borders::ALL).title(
            if screen.log_focus {
                " Progress / messages — focused "
            } else {
                " Progress / messages "
            },
        )),
        sections[2],
    );
    frame.render_widget(Paragraph::new("↑↓ Select  Space Mark  Enter Details  i Install  u Upgrade  U Self-upgrade\nS Sync catalog  c Check saved catalog  n Changelog  o Open  r Remove\na All apps  l Installed  Tab Focus  PgUp/PgDn Scroll  q / Esc Quit"),sections[3]);
    if let Some(popup) = &screen.popup {
        let rect = Rect::new(
            area.x + 2,
            area.y + 2,
            area.width.saturating_sub(4),
            area.height.saturating_sub(4),
        );
        frame.render_widget(Clear, rect);
        let (title, text, scroll) = match popup {
            Popup::Text {
                title,
                text,
                scroll,
            } => (
                title.as_str(),
                format!("{text}\n\nEsc/Enter Close  ↑↓/PgUp/PgDn Scroll"),
                *scroll,
            ),
            Popup::Confirm { text, scroll, .. } => (
                " Confirm transaction ",
                format!("Y Confirm   N / Esc Cancel   ↑↓/PgUp/PgDn Scroll\n\n{text}"),
                *scroll,
            ),
            Popup::Password { input, .. } => (
                " Authentication ",
                format!(
                    "Enter your sudo password.\n\n{}\n\nEnter Submit   Esc Cancel",
                    "*".repeat(input.chars().count())
                ),
                0,
            ),
        };
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(Block::default().borders(Borders::ALL).title(title)),
            rect,
        );
    }
}
pub(super) fn run(refresh: bool) -> Result<()> {
    ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "The terminal interface requires an interactive terminal"
    );
    enable_raw_mode()?;
    let _restore = RestoreTerminal;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    let (tx, rx): (Sender<Message>, Receiver<Message>) = mpsc::channel();
    let mut screen = Screen::new();
    screen.show_catalog = refresh;
    screen.busy = true;
    start(
        if refresh { Action::Sync } else { Action::Load },
        tx.clone(),
    );
    let mut dirty = true;
    loop {
        while let Ok(message) = rx.try_recv() {
            dirty = true;
            match message {
                Message::Log(text) => screen.log(text),
                Message::Catalog(rows) => screen.update(rows),
                Message::Text(title, text) => {
                    screen.popup = Some(Popup::Text {
                        title,
                        text: text.lines().map(clean_text).collect::<Vec<_>>().join("\n"),
                        scroll: 0,
                    })
                }
                Message::Confirm(text, reply) => {
                    screen.popup = Some(Popup::Confirm {
                        text,
                        scroll: 0,
                        reply,
                    })
                }
                Message::Password(reply) => {
                    screen.popup = Some(Popup::Password {
                        input: String::new(),
                        reply,
                    })
                }
                Message::Done(result) => {
                    screen.busy = false;
                    match result {
                        Ok(()) => {}
                        Err(e) => screen.log(format!("Error: {e:#}")),
                    };
                }
            }
        }
        if dirty {
            terminal.draw(|frame| draw(frame, &mut screen))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let input = event::read()?;
        if matches!(input, Event::Resize(_, _)) {
            dirty = true;
            continue;
        }
        let Event::Key(mut key) = input else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        dirty = true;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            key.code = KeyCode::Esc;
        }
        if let Some(popup) = &mut screen.popup {
            let mut close = false;
            match popup {
                Popup::Text { scroll, .. } => match key.code {
                    KeyCode::Esc | KeyCode::Enter => close = true,
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll = scroll.saturating_add(1),
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                    _ => {}
                },
                Popup::Confirm { reply, scroll, .. } => match key.code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll = scroll.saturating_add(1),
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                    KeyCode::Char('y' | 'Y') => {
                        let _ = reply.send(true);
                        close = true;
                    }
                    KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                        let _ = reply.send(false);
                        close = true;
                    }
                    _ => {}
                },
                Popup::Password { input, reply } => match key.code {
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(c)
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Enter => {
                        let password = std::mem::take(input);
                        let _ = reply.send(Some(password));
                        close = true;
                    }
                    KeyCode::Esc => {
                        let _ = reply.send(None);
                        close = true;
                    }
                    _ => {}
                },
            }
            if close {
                screen.popup = None;
            }
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if !screen.busy {
                    break;
                }
                screen.log("Wait for the current operation to finish.".into());
            }
            KeyCode::Char('a') | KeyCode::Char('l') => {
                screen.show_catalog = key.code == KeyCode::Char('a');
                screen.update(screen.all_entries.clone());
            }
            KeyCode::Tab => screen.log_focus = !screen.log_focus,
            KeyCode::Up if screen.log_focus => {
                screen.log_scroll = screen.log_scroll.saturating_add(1)
            }
            KeyCode::Down if screen.log_focus => {
                screen.log_scroll = screen.log_scroll.saturating_sub(1)
            }
            KeyCode::PageUp => screen.log_scroll = screen.log_scroll.saturating_add(5),
            KeyCode::PageDown => screen.log_scroll = screen.log_scroll.saturating_sub(5),
            KeyCode::Up => screen.move_selection(-1),
            KeyCode::Down => screen.move_selection(1),
            KeyCode::Char(' ') => {
                if let Some(e) = screen.current() {
                    let name = e.name.clone();
                    if !screen.marked.remove(&name) {
                        screen.marked.insert(name);
                    }
                }
            }
            KeyCode::Enter => {
                if screen.log_focus {
                    screen.popup = Some(Popup::Text {
                        title: "Progress / messages".into(),
                        text: screen.logs.join("\n"),
                        scroll: 0,
                    });
                } else if let Some(e) = screen.current() {
                    screen.popup = Some(Popup::Text {
                        title: e.name.clone(),
                        text: format!(
                            "{}\n\nInstalled: {}\nAvailable: {}\nStatus: {}\nUpstream: https://github.com/{}",
                            e.description,
                            e.local.as_deref().unwrap_or("Not installed"),
                            e.remote.as_deref().unwrap_or("Not published"),
                            e.status,
                            e.upstream
                        ),
                        scroll: 0,
                    });
                }
            }
            KeyCode::Char(c) if !screen.busy => {
                let targets = screen.targets();
                let action = match c {
                    'S' => {
                        screen.show_catalog = true;
                        Some(Action::Sync)
                    }
                    'c' => Some(Action::Check),
                    'i' if !targets.is_empty() => Some(Action::Install(targets)),
                    'u' => Some(Action::Upgrade(if screen.marked.is_empty() {
                        Vec::new()
                    } else {
                        targets
                    })),
                    'U' => Some(Action::Upgrade(vec!["artcraft".into()])),
                    'r' if !targets.is_empty() => Some(Action::Remove(targets)),
                    'n' => screen.current().map(|e| Action::Changelog(e.name.clone())),
                    'o' => screen.current().map(|e| Action::Open(e.name.clone())),
                    _ => None,
                };
                if let Some(action) = action {
                    screen.busy = true;
                    start(action, tx.clone());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// Pacman remains the package authority. TUI transactions show the resolved plan
// and request confirmation before any changes; every subprocess output stays
// in the screen. CLI transactions retain pacman's usual interactive behavior.
pub(super) fn transaction(arguments: &[String], feedback: &dyn Feedback) -> Result<()> {
    ensure!(
        Path::new("/etc/arch-release").exists(),
        "Installation requires Arch Linux or an Arch derivative"
    );
    let plan = Process::new("/usr/bin/pacman")
        .env("LC_ALL", "C")
        .args(["--print", "--print-format", "%n %v (%s bytes)"])
        .args(arguments)
        .output()?;
    ensure!(
        plan.status.success(),
        "Could not resolve transaction: {}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan = String::from_utf8(plan.stdout)?;
    ensure!(!plan.trim().is_empty(), "No packages in transaction");
    if !feedback.confirm(format!(
        "{}\n\n{}",
        if arguments.first().is_some_and(|a| a == "-R") {
            "Remove these packages?"
        } else {
            "Install / upgrade these packages?"
        },
        plan
    ))? {
        feedback.log("Transaction cancelled. No packages were changed.".into());
        return Ok(());
    }
    let uid = Process::new("/usr/bin/id").arg("-u").output()?;
    let root = String::from_utf8_lossy(&uid.stdout).trim() == "0";
    if !root
        && !Process::new("/usr/bin/sudo")
            .args(["-n", "-v"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success()
    {
        let Some(password) = feedback.password()? else {
            feedback.log("Authentication cancelled. No packages were changed.".into());
            return Ok(());
        };
        let mut child = Process::new("/usr/bin/sudo")
            .args(["-S", "-p", "", "-v"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = child
            .stdin
            .take()
            .context("Could not read authentication input")?;
        input.write_all(password.as_bytes())?;
        input.write_all(b"\n")?;
        drop(input);
        drop(password);
        let result = child.wait_with_output()?;
        ensure!(
            result.status.success(),
            "Authentication failed; no packages were changed"
        );
    }
    let mut command = if root {
        Process::new("/usr/bin/pacman")
    } else {
        let mut c = Process::new("/usr/bin/sudo");
        c.args(["-n", "--", "/usr/bin/pacman"]);
        c
    };
    let mut child = command
        .env("LC_ALL", "C")
        .args(["--noconfirm", "--color", "never"])
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("No package output")?;
    let stderr = child.stderr.take().context("No package error output")?;
    thread::scope(|scope| {
        scope.spawn(|| {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                feedback.log(line);
            }
        });
        scope.spawn(|| {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                feedback.log(line);
            }
        });
        ensure!(
            child.wait()?.success(),
            "Pacman did not complete the transaction"
        );
        Ok::<_, anyhow::Error>(())
    })?;
    feedback.log("Transaction completed.".into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    fn entry(name: &str, installed: bool) -> Entry {
        Entry {
            name: name.into(),
            description: "Application".into(),
            local: if installed {
                Some("1.0-1".into())
            } else {
                None
            },
            remote: Some("2.0-1".into()),
            size: Some(1024),
            upstream: "storytold/future".into(),
            status: if installed {
                "Upgrade available"
            } else {
                "Not installed"
            }
            .into(),
        }
    }
    #[test]
    fn terminal_defaults_and_versions_render_without_a_palette() {
        let mut screen = Screen::new();
        screen.update(vec![entry("future-tool", true)]);
        let backend = TestBackend::new(160, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut screen)).unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Installed version"));
        assert!(text.contains("Available version"));
        assert!(text.contains("1.0"));
        assert!(text.contains("2.0"));
        for cell in &buffer.content {
            assert_eq!(cell.fg, ratatui::style::Color::Reset);
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
            assert!(!cell.modifier.contains(Modifier::BOLD));
        }
    }
    #[test]
    fn installed_view_is_empty_until_an_actual_app_is_installed() {
        let mut screen = Screen::new();
        screen.update(vec![entry("future-tool", false)]);
        assert!(screen.entries.is_empty());
        assert!(screen.current().is_none());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| draw(f, &mut screen)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("No applications installed"));
        screen.show_catalog = true;
        screen.update(screen.all_entries.clone());
        assert_eq!(screen.entries.len(), 1);
        screen.show_catalog = false;
        screen.update(vec![entry("future-tool", true)]);
        assert_eq!(screen.current().unwrap().name, "future-tool");
    }
    #[test]
    fn switching_lists_preserves_selection_and_clears_hidden_marks() {
        let mut screen = Screen::new();
        screen.show_catalog = true;
        screen.update(vec![entry("first", false), entry("second", true)]);
        screen.move_selection(1);
        screen.marked.insert("first".into());
        screen.marked.insert("second".into());
        screen.show_catalog = false;
        screen.update(screen.all_entries.clone());
        assert_eq!(screen.current().unwrap().name, "second");
        assert_eq!(screen.targets(), vec!["second"]);
    }
    #[test]
    fn table_versions_hide_build_metadata_without_changing_details() {
        let full = "0.4.0.r20261009150246.g7b6c13447228-1";
        assert_eq!(short_version(Some(full)), "0.4.0");
        assert_eq!(short_version(Some("1.2.3-2")), "1.2.3");
        assert_eq!(short_version(None), "—");
        assert_eq!(short_version(Some("1.2.rc1-1")), "1.2.rc1");
        let mut item = entry("future-tool", true);
        item.local = Some(full.into());
        let mut screen = Screen::new();
        screen.update(vec![item]);
        assert_eq!(screen.current().unwrap().local.as_deref(), Some(full));
    }
    #[test]
    fn logs_cannot_inject_terminal_escape_sequences() {
        assert_eq!(clean_text("\u{1b}[31mhello\u{1b}[0m"), "hello");
    }
}
