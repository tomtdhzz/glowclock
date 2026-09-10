# glowclock

[中文说明](./README.zh-CN.md)

A gradient big-digit terminal clock (tty-clock style) with **crontab-style reminders** and a **fat cat**. When a reminder is due, the cat pops up (e.g. "drink water" every hour) and the terminal bell rings.

Built with Rust + [ratatui](https://ratatui.rs) + crossterm — only those two dependencies. Local time comes from the OS timezone offset (`date +%z`); no `chrono`.

```
 ┌ GLOWCLOCK  theme: aurora ┐
        Thu 2026-09-10
    ██████   ██████   ...        # big digits with a top→bottom colour gradient
    q quit  space theme  ...
```

## Build & run

```bash
cd glowclock
cargo build --release
./target/release/glowclock            # start the interactive clock
```

On start, if a `glowclock-reminders.txt` exists in the working directory it is loaded automatically; otherwise the built-in defaults are used (drink water hourly + look away every 45 min).

## Keys

| Key           | Action                        |
| ------------- | ----------------------------- |
| `q` / `Esc`   | Quit                          |
| `a`           | Add a reminder (type it in)   |
| `space` / `c` | Cycle theme                   |
| `f`           | Toggle 12/24-hour format      |
| any key       | Dismiss the current cat popup |

> While a popup is showing, any key except `q` (which always quits) dismisses it first.

## The clock

- Big digits are scaled from a built-in 5×7 bitmap font, coloured with a **vertical gradient** (bright on top, deep at the bottom) — that is the "glow".
- The **colon is steady** (no blink); only the digits change. It is a slim 1-px pair of dots, so it never crowds the digits.
- **Responsive sizing**: the font scale adapts to the window, always keeping a gap between digits so they never mash together.
- **Narrow-window fallback**: if the full `HH:MM:SS` cannot fit even at the smallest scale, the colons are dropped automatically (digits stay gapped and legible).
- The date and weekday are shown above the clock, e.g. `Thu 2026-09-10`.

### Themes

Four built in: `aurora` (default, teal→blue), `sunset` (orange→pink), `matrix` (green), `ice` (cool white→blue-grey).

```bash
./target/release/glowclock --theme sunset
./target/release/glowclock --theme 2       # index also works
./target/release/glowclock --gallery       # preview all themes (to stdout)
```

Press `space` at runtime to cycle.

## Reminders (crontab style)

### File format

One reminder per line; blank lines and lines starting with `#` are ignored. Each line is a **schedule** followed by the **message**.

**1) Classic 5-field cron**

```
min  hour  dom  mon  dow   message
```

| Field | Range | Notes                                   |
| ----- | ----- | --------------------------------------- |
| min   | 0–59  |                                         |
| hour  | 0–23  |                                         |
| dom   | 1–31  | day of month                            |
| mon   | 1–12  |                                         |
| dow   | 0–7   | 0 and 7 are Sunday, 1=Mon … 6=Sat       |

Each field supports `*`, `N`, `A-B` (range), `*/S` (step), `A-B/S`, and `a,b,c` (list).

> As in standard cron: when both `dom` and `dow` are restricted (not `*`), a match on **either** fires; if one is `*`, the other decides.

**2) Convenience keywords**

```
@minutely   message      # = * * * * *
@hourly     message      # = 0 * * * *
@daily      message      # = 0 0 * * *
```

**3) Fixed interval**

```
@every <duration>   message
```

Duration: `90s`, `30m`, `1h`, `1h30m`, `2d`, or a bare number (seconds). Counted from launch, then every interval after.

### Example

```crontab
# min hour dom mon dow   message
0 * * * *          time to drink water! stretch a bit (=^.^=)
*/30 9-18 * * 1-5  workday: rest your neck, look far away
0 12 * * *         lunch time, feed the cat
0 18 * * 1-5       before leaving: review your day
@every 45m         blink and look away from the screen
```

The repo ships `glowclock-reminders.txt` as a ready-to-edit example.

### Load order

The first available source wins:

1. the file given by `--reminders <path>`
2. `glowclock-reminders.txt` in the current directory
3. `~/.config/glowclock/reminders.txt`
4. built-in defaults

```bash
./target/release/glowclock --reminders ~/my-reminders.txt
./target/release/glowclock --list-reminders   # print what is loaded (source + parse errors)
```

Lines that fail to parse are reported to stderr on start and skipped; other lines still load.

### Manage reminders from the CLI

Instead of editing the file by hand, add / list / remove reminders directly. These write the reminders file (see *target file* below).

You can also add one **without leaving the clock**: press `a`, type the reminder (same format as a file line — no shell quoting needed here), and press Enter. It is saved to the file and starts firing immediately; the cat confirms with `已添加提醒：…`.

```bash
# add:  glowclock add <schedule> <message...>
glowclock add @hourly drink water            # convenience keyword, no quoting
glowclock add @every 45m look away           # interval, no quoting
glowclock add "0 9 * * 1-5" morning standup  # raw cron: QUOTE it, or the shell eats the '*'

glowclock list                               # show reminders with an index
glowclock rm 2                               # remove reminder #2
```

> Quote any raw cron expression (`"0 9 * * 1-5"`) — otherwise the shell expands `*` into filenames. `@hourly`/`@daily`/`@every` need no quoting.

**Target file** (which file `add`/`rm` write to): the `--reminders <path>` file if given; else an existing default file; else `~/.config/glowclock/reminders.txt` (created for you). Put `--reminders <path>` before the subcommand to target a specific file:

```bash
glowclock --reminders ~/my-reminders.txt add @hourly drink water
```

### How reminders appear / how to dismiss

- **Appear**: a rounded box `╭ 胖猫 提醒 ╮` pops up in the center with the cat, the message, and `按任意键关闭` ("press any key to close"); the terminal bell rings.
- **Fire once**: a cron reminder fires once during its matching minute; `@every` fires once per period.
- **Dismiss**: press any key to close immediately, or it **auto-closes after 60 seconds**. A visible popup is not interrupted by the next reminder.
- **Quit** the app with `q` or `Esc`.

## Defining the cat

### Pick a built-in

Built in: `chonk` (default), `kitten`, `loaf`, `sleepy`, `peek`.

```bash
./target/release/glowclock --cat kitten
./target/release/glowclock --list-cats            # preview the default cat
./target/release/glowclock --list-cats --cat loaf # preview a specific cat
```

### Use your own

Point `--cat-file` at a text file; **each line is one row of the cat** (shown verbatim, any Unicode/ASCII):

```bash
./target/release/glowclock --cat-file ~/mycat.txt
```

`mycat.txt`:

```
 /\_/\
( o.o )
 > ^ <
```

> The popup sizes itself to the cat and message; CJK full-width characters count as two columns. Keep rows a similar width for a tidy border. `--cat-file` overrides `--cat`; with neither, the default `chonk` is used.

## CLI reference

```
glowclock [options]
glowclock <subcommand> ...

subcommands (manage the reminders file):
  add <schedule> <message...>   add a reminder (quote raw cron: "0 9 * * 1-5")
  list                          list reminders with an index
  rm <index>                    remove the reminder at <index>

modes (default: interactive clock):
  --snapshot          write one truecolor frame to stdout, then exit
  --gallery           write one frame per theme, then exit
  --plain             write one monochrome block frame (no ANSI)
  --list-reminders    print loaded reminders (source, entries, errors), then exit
  --list-cats         print the cat that would be used (with --cat/--cat-file)

options:
  --theme <name|N>    aurora sunset matrix ice (or index 0..3)
  --time HH:MM:SS      render a fixed time (for screenshots/debug)
  --reminders <path>   load reminders from a crontab-style file
  --cat <name>         built-in cat: chonk kitten loaf sleepy peek
  --cat-file <path>    load a custom cat from a file (one row per line)
  --12 | --24          hour format (default 24)
  -h, --help           help
```

## How it works

- **Time/date** (`src/clock.rs`): the timezone offset is read once from `date +%z`; year/month/day/weekday are derived from the UNIX timestamp with Howard Hinnant's civil-date algorithm — no `chrono`.
- **Font & gradient** (`src/font.rs`, `src/render.rs`): a 5×7 bitmap scaled up, with per-row RGB interpolation and adaptive sizing.
- **Reminders** (`src/reminder.rs`): a small cron field parser (bitmask matching) plus `@every` interval scheduling and once-per-occurrence de-duplication.
- **Cat** (`src/mascot.rs`): several built-in ASCII cats, selectable by name or loaded from a file.
- **UI** (`src/main.rs`): ratatui layout with a `Clear` overlay for the popup.

### Tests

```bash
cargo test --all
```

Covers civil-date/weekday conversion, cron parsing and matching (step/range/list, Sunday 0-7, dom-or-dow semantics), duration parsing, once-per-occurrence firing for both cron and interval, adaptive scaling (and the "never zero-gap" invariant), gradient colour selection, and cat resolution.

## License

MIT © 2026 tangdihui
