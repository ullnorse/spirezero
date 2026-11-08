//! The boss screen: plain, colourless text that looks like work. Nothing on it can be clicked;
//! any key or click brings the game back.

use std::fmt::Write as _;

use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};

use crate::settings::BossView;

/// Crates a build log compiles, with their versions.
const CRATES: &[&str] = &[
    "proc-macro2 v1.0.95",
    "unicode-ident v1.0.18",
    "libc v0.2.172",
    "cfg-if v1.0.0",
    "autocfg v1.4.0",
    "once_cell v1.21.3",
    "memchr v2.7.4",
    "itoa v1.0.15",
    "ryu v1.0.20",
    "smallvec v1.15.0",
    "log v0.4.27",
    "bitflags v2.9.1",
    "pin-project-lite v0.2.16",
    "quote v1.0.40",
    "syn v2.0.101",
    "serde_derive v1.0.219",
    "serde v1.0.219",
    "serde_json v1.0.140",
    "bytes v1.10.1",
    "futures-core v0.3.31",
    "tracing-core v0.1.33",
    "mio v1.0.3",
    "socket2 v0.5.9",
    "tokio-macros v2.5.0",
    "tokio v1.45.0",
    "http v1.3.1",
    "httparse v1.10.1",
    "h2 v0.4.10",
    "tower-service v0.3.3",
    "hyper v1.6.0",
    "rustls v0.23.27",
    "ring v0.17.14",
    "webpki-roots v1.0.0",
    "regex-syntax v0.8.5",
    "aho-corasick v1.1.3",
    "regex-automata v0.4.9",
    "regex v1.11.1",
    "chrono v0.4.41",
    "uuid v1.16.0",
    "sqlx-core v0.8.6",
    "sqlx-postgres v0.8.6",
    "sqlx-macros v0.8.6",
    "tracing v0.1.41",
    "tracing-subscriber v0.3.19",
    "axum-core v0.5.2",
    "axum v0.8.4",
    "billing-core v0.14.2 (/srv/build/billing/crates/core)",
    "billing-ledger v0.14.2 (/srv/build/billing/crates/ledger)",
    "billing-api v0.14.2 (/srv/build/billing/crates/api)",
];

/// Processes a `top` lists: command, user (`None`: the player), CPU and memory tenths.
const PROCESSES: &[(&str, Option<&str>, u32, u32)] = &[
    ("rust-analyzer", None, 112, 41),
    ("code", None, 64, 52),
    ("firefox", None, 38, 87),
    ("Xorg", Some("root"), 21, 9),
    ("pipewire", None, 9, 2),
    ("dockerd", Some("root"), 7, 18),
    ("postgres", Some("postgres"), 5, 33),
    ("slack", None, 4, 61),
    ("systemd", Some("root"), 1, 1),
    ("sshd", Some("root"), 0, 1),
    ("kworker/2:1-events", Some("root"), 0, 0),
    ("cron", Some("root"), 0, 0),
    ("bash", None, 0, 1),
    ("node", None, 2, 24),
    ("containerd", Some("root"), 3, 7),
    ("journald", Some("root"), 1, 3),
    ("NetworkManager", Some("root"), 0, 2),
    ("gnome-shell", None, 12, 29),
    ("redis-server", Some("redis"), 1, 4),
    ("ssh-agent", None, 0, 0),
];

/// Draws the boss screen `view` over the whole frame.
pub fn draw(frame: &mut Frame<'_>, view: BossView) {
    let area = frame.area();
    let rows = usize::from(area.height);
    let lines = match view {
        BossView::Build => build_log(rows),
        BossView::Top => top(rows),
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>()),
        area,
    );
}

/// The last `rows` lines of a build in progress.
fn build_log(rows: usize) -> Vec<String> {
    let done = CRATES.len() * 2 / 3;
    let mut lines: Vec<String> = [
        "$ cargo build --workspace --release".to_owned(),
        "    Updating crates.io index".to_owned(),
    ]
    .into_iter()
    .chain(CRATES[..done].iter().map(|c| format!("   Compiling {c}")))
    .collect();
    let total = 214;
    let at = 187;
    let bar = 27 * at / total;
    let progress = format!(
        "    Building [{}>{}] {at}/{total}: {}",
        "=".repeat(bar),
        " ".repeat(27 - bar),
        CRATES[done..]
            .iter()
            .take(4)
            .filter_map(|c| c.split(' ').next())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let keep = rows.saturating_sub(1);
    let skip = lines.len().saturating_sub(keep);
    lines.drain(..skip);
    lines.push(progress);
    lines
}

/// A `top` screen of `rows` lines.
fn top(rows: usize) -> Vec<String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (h, m, s) = (seconds / 3600 % 24, seconds / 60 % 60, seconds % 60);
    let user: String = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "dev".to_owned())
        .chars()
        .take(8)
        .collect();
    let mut lines = vec![
        format!(
            "top - {h:02}:{m:02}:{s:02} up 12 days,  3:41,  1 user,  load average: 0.52, 0.58, 0.59"
        ),
        "Tasks: 287 total,   1 running, 286 sleeping,   0 stopped,   0 zombie".to_owned(),
        "%Cpu(s):  3.1 us,  1.0 sy,  0.0 ni, 95.6 id,  0.2 wa,  0.0 hi,  0.1 si,  0.0 st"
            .to_owned(),
        "MiB Mem :  31842.1 total,  12011.4 free,   9123.8 used,  10706.9 buff/cache".to_owned(),
        "MiB Swap:   8192.0 total,   8192.0 free,      0.0 used.  22014.7 avail Mem".to_owned(),
        String::new(),
        "    PID USER      PR  NI    VIRT    RES    SHR S  %CPU  %MEM     TIME+ COMMAND".to_owned(),
    ];
    for (i, &(command, owner, cpu, mem)) in PROCESSES.iter().enumerate() {
        if lines.len() >= rows {
            break;
        }
        let pid = 1000 + i * 1789 % 9000;
        let owner = owner.unwrap_or(&user);
        let state = if i == 0 { 'R' } else { 'S' };
        let mut line = String::new();
        let _ = write!(
            line,
            "{pid:>7} {owner:<9} 20   0 {:>7} {:>6} {:>6} {state} {:>3}.{} {:>3}.{} {:>3}:{:02}.{:02} {command}",
            format!("{}.{}g", 1 + i % 9, i % 10),
            200_000 + i * 37_119 % 900_000,
            40_000 + i * 7_717 % 90_000,
            cpu / 10,
            cpu % 10,
            mem / 10,
            mem % 10,
            i * 7 % 60,
            i * 13 % 60,
            i * 29 % 100,
        );
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_views_fill_the_screen_without_running_over() {
        for rows in [10, 24, 60] {
            assert_eq!(build_log(rows).len(), rows.min(CRATES.len() * 2 / 3 + 3));
            assert!(top(rows).len() <= rows);
        }
    }
}
