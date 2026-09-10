//! glowclock — a gradient big-digit terminal clock with crontab-style
//! reminders and a 胖猫 (fat-cat) popup.
//!
//! Default: an interactive clock. Reminders fire a cat popup (e.g. hourly
//! "drink water"). Non-interactive modes exist for capture/inspection:
//!   --snapshot          one frame of truecolor ANSI to stdout, then exit
//!   --gallery           one frame per built-in theme, then exit
//!   --plain             monochrome block frame (no ANSI)
//!   --list-reminders    print the loaded reminders, then exit
//!   --theme <name|N>    pick a theme (default: aurora / 0)
//!   --time HH:MM:SS     render a fixed time instead of "now"
//!   --reminders <path>  load reminders from a crontab-style file
//!   --12 | --24         hour format (default 24h)

mod clock;
mod font;
mod mascot;
mod reminder;
mod render;

use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use reminder::{Manager, Reminder};
use render::{Theme, THEMES};

struct Args {
    mode: Mode,
    theme: usize,
    fixed: Option<String>,
    hour24: bool,
    reminders_path: Option<String>,
    cat_name: Option<String>,
    cat_file: Option<String>,
}

enum Mode {
    Interactive,
    Snapshot,
    Gallery,
    Plain,
    ListReminders,
    ListCats,
    Add(String),
    Rm(usize),
}

fn main() {
    let mut args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };

    let mode = std::mem::replace(&mut args.mode, Mode::Interactive);
    let result = match mode {
        Mode::Interactive => run_interactive(args),
        Mode::Snapshot => run_oneshot(args, true),
        Mode::Gallery => run_gallery(args),
        Mode::Plain => run_oneshot(args, false),
        Mode::ListReminders => run_list_reminders(args),
        Mode::ListCats => run_list_cats(args),
        Mode::Add(line) => run_add(args, line),
        Mode::Rm(index) => run_rm(args, index),
    };

    if let Err(e) = result {
        eprintln!("glowclock: {e}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<Args, String> {
    let mut mode = Mode::Interactive;
    let mut theme = 0usize;
    let mut fixed = None;
    let mut hour24 = true;
    let mut reminders_path = None;
    let mut cat_name = None;
    let mut cat_file = None;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--snapshot" => mode = Mode::Snapshot,
            "--gallery" => mode = Mode::Gallery,
            "--plain" => mode = Mode::Plain,
            "--list-reminders" | "list" => mode = Mode::ListReminders,
            "--list-cats" => mode = Mode::ListCats,
            "add" => {
                let line = it.by_ref().collect::<Vec<String>>().join(" ");
                mode = Mode::Add(line);
            }
            "rm" | "remove" => {
                let n = it
                    .next()
                    .ok_or("rm needs an index (see `glowclock list`)")?;
                let idx: usize = n.parse().map_err(|_| format!("invalid index: {n}"))?;
                mode = Mode::Rm(idx);
            }
            "--24" => hour24 = true,
            "--12" => hour24 = false,
            "--theme" => theme = resolve_theme(&it.next().ok_or("--theme needs a value")?)?,
            "--time" => fixed = Some(it.next().ok_or("--time needs HH:MM:SS")?),
            "--reminders" => reminders_path = Some(it.next().ok_or("--reminders needs a path")?),
            "--cat" => cat_name = Some(it.next().ok_or("--cat needs a name")?),
            "--cat-file" => cat_file = Some(it.next().ok_or("--cat-file needs a path")?),
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        mode,
        theme,
        fixed,
        hour24,
        reminders_path,
        cat_name,
        cat_file,
    })
}

fn resolve_theme(v: &str) -> Result<usize, String> {
    if let Ok(n) = v.parse::<usize>() {
        if n < THEMES.len() {
            return Ok(n);
        }
        return Err(format!("theme index out of range (0..{})", THEMES.len()));
    }
    THEMES
        .iter()
        .position(|t| t.name == v)
        .ok_or_else(|| format!("unknown theme: {v}"))
}

fn print_help() {
    println!("glowclock — gradient clock with crontab-style reminders and a fat cat\n");
    println!(
        "USAGE: glowclock [--snapshot|--gallery|--plain|--list-reminders|--list-cats]\n       \
         [--theme <name|N>] [--time HH:MM:SS] [--reminders <path>]\n       \
         [--cat <name>] [--cat-file <path>] [--12|--24]\n"
    );
    print!("themes:");
    for (i, t) in THEMES.iter().enumerate() {
        print!(" {i}:{}", t.name);
    }
    print!("\ncats:  ");
    print!("{}", mascot::NAMES.join(", "));
    println!("   (default: {})", mascot::default_name());
    println!("\nreminders file (crontab-style, one per line):");
    println!("  min hour dom mon dow  message   |  @hourly/@daily/@every <dur>  message");
    println!("\nmanage reminders from the CLI (writes the reminders file):");
    println!("  glowclock add <schedule> <message...>   add one (quote cron so the shell");
    println!("                                          keeps the '*', e.g. \"0 9 * * 1-5\")");
    println!("  glowclock list                          list reminders with an index");
    println!("  glowclock rm <index>                    remove the reminder at <index>");
    println!(
        "\nkeys (interactive): q/Esc quit · space/c theme · f 12/24h · any key closes a popup"
    );
}

fn run_list_cats(args: Args) -> io::Result<()> {
    match mascot::resolve(args.cat_name.as_deref(), args.cat_file.as_deref()) {
        Ok(cat) => {
            let label = args
                .cat_file
                .as_deref()
                .map(|p| format!("file:{p}"))
                .or(args.cat_name.clone())
                .unwrap_or_else(|| mascot::default_name().to_string());
            println!("cat: {label}");
            for line in &cat {
                println!("{line}");
            }
            println!("\navailable built-in cats: {}", mascot::NAMES.join(", "));
        }
        Err(e) => {
            eprintln!("glowclock: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}

// ---- reminder loading -------------------------------------------------------

/// Load reminders: explicit `--reminders` path, else the first existing default
/// file, else the built-in defaults. Returns `(reminders, source, errors)`.
fn load_reminders(path: Option<&str>) -> (Vec<Reminder>, String, Vec<String>) {
    if let Some(p) = path {
        match std::fs::read_to_string(p) {
            Ok(text) => {
                let lines: Vec<String> = text.lines().map(str::to_string).collect();
                let (rs, errs) = reminder::parse_all(&lines);
                return (rs, p.to_string(), errs);
            }
            Err(e) => {
                return (
                    Vec::new(),
                    p.to_string(),
                    vec![format!("cannot read {p}: {e}")],
                );
            }
        }
    }
    for candidate in default_paths() {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            let (rs, errs) = reminder::parse_all(&lines);
            return (rs, candidate, errs);
        }
    }
    let lines: Vec<String> = reminder::default_lines()
        .into_iter()
        .map(str::to_string)
        .collect();
    let (rs, errs) = reminder::parse_all(&lines);
    (rs, "built-in defaults".to_string(), errs)
}

fn default_paths() -> Vec<String> {
    let mut v = vec!["glowclock-reminders.txt".to_string()];
    if let Ok(home) = std::env::var("HOME") {
        v.push(format!("{home}/.config/glowclock/reminders.txt"));
    }
    v
}

/// The file that `add`/`rm` should modify: explicit `--reminders`, else an
/// existing default file, else the user config path (created on demand).
fn mutate_target(path: Option<&str>) -> String {
    if let Some(p) = path {
        return p.to_string();
    }
    for candidate in default_paths() {
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    // None exist yet: prefer the user config path, fall back to the cwd file.
    default_paths()
        .pop()
        .unwrap_or_else(|| "glowclock-reminders.txt".to_string())
}

/// `glowclock add <schedule> <message...>` — validate and append one reminder.
fn run_add(args: Args, line: String) -> io::Result<()> {
    let line = line.trim().to_string();
    if line.is_empty() {
        eprintln!("glowclock: add needs a schedule and a message");
        eprintln!("  e.g. glowclock add 0 9 * * 1-5 开晨会");
        eprintln!("       glowclock add @every 45m 远眺");
        std::process::exit(2);
    }
    match reminder::Reminder::parse(&line) {
        Ok(Some(r)) if !r.message.trim().is_empty() => {}
        Ok(_) => {
            eprintln!("glowclock: that reminder has no message text");
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("glowclock: invalid schedule: {e}");
            std::process::exit(2);
        }
    }

    let target = mutate_target(args.reminders_path.as_deref());
    if let Some(parent) = std::path::Path::new(&target).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut text = std::fs::read_to_string(&target).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&line);
    text.push('\n');
    std::fs::write(&target, text)?;
    println!("added to {target}:\n  {line}");
    Ok(())
}

/// `glowclock rm <index>` — remove the Nth reminder (as shown by `list`).
fn run_rm(args: Args, index: usize) -> io::Result<()> {
    let target = mutate_target(args.reminders_path.as_deref());
    let text = match std::fs::read_to_string(&target) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("glowclock: cannot read {target}: {e}");
            std::process::exit(1);
        }
    };
    if index == 0 {
        eprintln!("glowclock: index starts at 1 (see `glowclock list`)");
        std::process::exit(2);
    }

    // Walk lines, counting reminder entries; drop the one at `index`.
    let mut count = 0usize;
    let mut removed: Option<String> = None;
    let mut kept: Vec<String> = Vec::new();
    for line in text.lines() {
        let is_entry = matches!(reminder::Reminder::parse(line), Ok(Some(_)));
        if is_entry {
            count += 1;
            if count == index {
                removed = Some(line.to_string());
                continue; // skip it
            }
        }
        kept.push(line.to_string());
    }

    match removed {
        Some(line) => {
            let mut out = kept.join("\n");
            if !out.is_empty() {
                out.push('\n');
            }
            std::fs::write(&target, out)?;
            println!("removed from {target}:\n  {line}");
            Ok(())
        }
        None => {
            eprintln!("glowclock: no reminder #{index} in {target} (have {count})");
            std::process::exit(1);
        }
    }
}

fn run_list_reminders(args: Args) -> io::Result<()> {
    let (rs, source, errs) = load_reminders(args.reminders_path.as_deref());
    println!("source: {source}");
    if !errs.is_empty() {
        println!("errors:");
        for e in &errs {
            println!("  {e}");
        }
    }
    println!("reminders ({}):", rs.len());
    for (i, r) in rs.iter().enumerate() {
        println!("  {}. {}", i + 1, r.source);
    }
    if !rs.is_empty() {
        println!("\nremove one with: glowclock rm <index>");
    }
    Ok(())
}

// ---- non-interactive render modes ------------------------------------------

/// The time string to display: fixed override, or "now" (steady colon).
fn current_display(args: &Args) -> String {
    if let Some(f) = &args.fixed {
        return f.clone();
    }
    let offset = clock::local_utc_offset_seconds();
    clock::format_display(clock::now_local(offset), args.hour24)
}

fn run_oneshot(args: Args, color: bool) -> io::Result<()> {
    let text = current_display(&args);
    let theme = THEMES[args.theme];
    let mut out = io::stdout().lock();
    if color {
        render::write_ansi(&mut out, &text, theme, render::DEFAULT_SCALE)?;
    } else {
        write_plain(&mut out, &text, theme)?;
    }
    Ok(())
}

fn run_gallery(args: Args) -> io::Result<()> {
    let text = args.fixed.clone().unwrap_or_else(|| current_display(&args));
    let mut out = io::stdout().lock();
    for theme in THEMES {
        writeln!(out, "\x1b[1m {} \x1b[0m", theme.name)?;
        render::write_ansi(&mut out, &text, *theme, render::DEFAULT_SCALE)?;
        writeln!(out)?;
    }
    Ok(())
}

/// Monochrome block frame with no ANSI colour — stays readable in plain logs.
fn write_plain(out: &mut impl Write, text: &str, theme: Theme) -> io::Result<()> {
    for line in render::clock_lines(text, theme, render::DEFAULT_SCALE) {
        let row: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        writeln!(out, "{}", row.trim_end())?;
    }
    Ok(())
}

// ---- interactive ------------------------------------------------------------

/// State of the active fat-cat popup, if any.
struct Popup {
    message: String,
    shown_at: i64,
}

/// How long a popup stays before auto-dismissing.
const POPUP_TTL_SECS: i64 = 60;

fn run_interactive(mut args: Args) -> io::Result<()> {
    let offset = clock::local_utc_offset_seconds();
    let mut theme_idx = args.theme;

    let (reminders, source, errs) = load_reminders(args.reminders_path.as_deref());
    for e in &errs {
        eprintln!("glowclock: reminder {e}");
    }
    let mut manager = Manager::new(reminders, clock::now_unix());
    let _ = source;

    let cat = match mascot::resolve(args.cat_name.as_deref(), args.cat_file.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("glowclock: {e}");
            std::process::exit(1);
        }
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let res = interactive_loop(
        &mut term,
        &mut args,
        &mut theme_idx,
        offset,
        &mut manager,
        &cat,
    );

    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    term.show_cursor().ok();
    res
}

fn interactive_loop<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    args: &mut Args,
    theme_idx: &mut usize,
    offset: i32,
    manager: &mut Manager,
    cat: &[String],
) -> io::Result<()> {
    let mut popup: Option<Popup> = None;
    loop {
        let now = clock::now_unix();
        let dt = clock::now_datetime(offset);

        // Fire at most one popup per tick; a live popup is not interrupted.
        if popup.is_none() {
            if let Some(message) = manager.poll(&dt, now) {
                ring_bell();
                popup = Some(Popup {
                    message,
                    shown_at: now,
                });
            }
        } else if let Some(p) = &popup {
            if now - p.shown_at >= POPUP_TTL_SECS {
                popup = None;
            }
        }

        let text = match &args.fixed {
            Some(f) => f.clone(),
            None => clock::format_display(dt.hms(), args.hour24),
        };
        let theme = THEMES[*theme_idx];
        term.draw(|f| {
            ui(
                f,
                &text,
                &dt,
                theme,
                args.hour24,
                manager,
                popup.as_ref(),
                cat,
            )
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Release {
                    match k.code {
                        KeyCode::Char('q') => break,
                        _ if popup.is_some() => popup = None,
                        KeyCode::Esc => break,
                        KeyCode::Char('c') | KeyCode::Char(' ') => {
                            *theme_idx = (*theme_idx + 1) % THEMES.len();
                        }
                        KeyCode::Char('f') => args.hour24 = !args.hour24,
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn ring_bell() {
    let mut out = io::stdout();
    let _ = out.write_all(b"\x07");
    let _ = out.flush();
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

#[allow(clippy::too_many_arguments)]
fn ui(
    f: &mut Frame,
    text: &str,
    dt: &clock::DateTime,
    theme: Theme,
    hour24: bool,
    manager: &Manager,
    popup: Option<&Popup>,
    cat: &[String],
) {
    let area = f.area();
    let bg = Style::default().bg(rgb(theme.bg));
    f.render_widget(Block::default().style(bg), area);

    let rows = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(1), // date
        Constraint::Min(1),    // clock
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Title chip + theme label.
    let title = Line::from(vec![
        Span::styled(
            " GLOWCLOCK ",
            Style::default()
                .fg(rgb(theme.bg))
                .bg(rgb(theme.top))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("theme: {}", theme.name),
            Style::default()
                .fg(rgb(theme.top))
                .add_modifier(Modifier::DIM),
        ),
    ]);
    f.render_widget(
        Paragraph::new(title).alignment(Alignment::Center).style(bg),
        rows[0],
    );

    // Date line.
    let date = Line::from(Span::styled(
        format!(
            "{} {:04}-{:02}-{:02}",
            clock::weekday_name(dt.dow),
            dt.year,
            dt.month,
            dt.day
        ),
        Style::default()
            .fg(rgb(theme.top))
            .add_modifier(Modifier::DIM),
    ));
    f.render_widget(
        Paragraph::new(date).alignment(Alignment::Center).style(bg),
        rows[1],
    );

    // Big clock — modest scale that leaves a margin, vertically centered.
    // The slim colon plus the always-gapped scale ladder keep the separators
    // readable; only when even the smallest scale overflows do we drop the
    // colons entirely (digits stay gapped, so still legible).
    let body = rows[2];
    let max_w = (body.width as usize).saturating_sub(4);
    let max_h = body.height as usize;
    let scale = render::best_scale(text, max_w, max_h);
    let (display, scale) = if render::clock_width(text, scale) <= max_w {
        (text.to_string(), scale)
    } else {
        let compact: String = text.chars().filter(|c| *c != ':').collect();
        let s = render::best_scale(&compact, max_w, max_h);
        (compact, s)
    };
    let mut lines = render::clock_lines(&display, theme, scale);
    let pad = (body.height as usize).saturating_sub(lines.len()) / 2;
    let mut framed: Vec<Line> = vec![Line::from(""); pad];
    framed.append(&mut lines);
    f.render_widget(
        Paragraph::new(framed)
            .alignment(Alignment::Center)
            .style(bg),
        body,
    );

    // Footer hints.
    let dim = Style::default()
        .fg(rgb(theme.top))
        .add_modifier(Modifier::DIM);
    let key = Style::default().fg(rgb(theme.top));
    let footer = Line::from(vec![
        Span::styled("q", key),
        Span::styled(" quit  ", dim),
        Span::styled("space", key),
        Span::styled(" theme  ", dim),
        Span::styled("f", key),
        Span::styled(format!(" {}h  ", if hour24 { "24" } else { "12" }), dim),
        Span::styled("🐱", key),
        Span::styled(format!(" {} reminders", reminder_count(manager)), dim),
    ]);
    f.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(bg),
        rows[3],
    );

    // Fat-cat popup overlay.
    if let Some(p) = popup {
        render_popup(f, area, theme, &p.message, cat);
    }
}

fn reminder_count(manager: &Manager) -> &'static str {
    // Manager owns the count but exposes only emptiness; keep the footer honest
    // without leaking internals.
    if manager.is_empty() {
        "no"
    } else {
        "active"
    }
}

fn render_popup(f: &mut Frame, area: Rect, theme: Theme, message: &str, cat: &[String]) {
    let inner_w = mascot::width(cat).max(mascot::disp_width(message).min(32));
    let w = (inner_w as u16 + 8)
        .min(area.width.saturating_sub(2))
        .max(20);
    let h = (cat.len() as u16 + 7).min(area.height.saturating_sub(2));
    let rect = centered_rect(w, h, area);

    let mut content: Vec<Line> = Vec::new();
    for l in cat {
        content.push(Line::from(Span::styled(
            l.clone(),
            Style::default().fg(rgb(theme.top)),
        )));
    }
    content.push(Line::from(""));
    content.push(Line::from(Span::styled(
        message.to_string(),
        Style::default()
            .fg(Color::Rgb(245, 245, 250))
            .add_modifier(Modifier::BOLD),
    )));
    content.push(Line::from(""));
    content.push(Line::from(Span::styled(
        "按任意键关闭",
        Style::default()
            .fg(rgb(theme.top))
            .add_modifier(Modifier::DIM),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.top)))
        .title(" 胖猫 提醒 ")
        .title_style(
            Style::default()
                .fg(rgb(theme.bg))
                .bg(rgb(theme.top))
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(rgb(theme.bg)));

    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rect,
    );
}

/// A `w` x `h` rectangle centered within `area` (clamped to fit).
fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}
