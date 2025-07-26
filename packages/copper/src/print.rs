use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant};

use unicode_width::UnicodeWidthChar;

static YELLOW: &str = "\x1b[1;33m";
static RESET: &str = "\x1b[0m";

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum PrintLevel {
    QuietQuiet,
    Quiet,
    Normal,
    Verbose,
    VerboseVerbose
}


static GLOBAL_PRINT: LazyLock<Mutex<Printer>> = LazyLock::new(|| Mutex::new(Printer::default()));


thread_local! {
    static THREAD_NAME: RefCell<Option<String>> = RefCell::new(None);
}

struct Printer {
    is_terminal: bool,
    level: PrintLevel,
    prompt_active: bool,
    bars: Vec<Weak<Mutex<ProgressBar>>>,

    buffered: String,
}

// struct PrintThreadHandle {
//     send: Sender<Arc<Mutex<ProgressBar>>>,
//     stop: Arc<AtomicBool>,
// }

impl Default for Printer {
    fn default() -> Self {
        todo!()
    }
}

impl Printer {
    fn print_internal(&mut self, x: &str) {
        if !self.prompt_active && self.bars.is_empty() {
            print!("{x}");
            return;
        }
        self.buffered.push_str(x);
    }

    fn add_progress_bar(&mut self, bar: Arc<Mutex<ProgressBar>>) {
        if self.level < PrintLevel::Normal {
            return;
        }
        if !self.is_terminal {
            return;
        }
        self.bars.push(Arc::downgrade(&bar));
    }

    fn print_progress_bar_done(&mut self, total: u64, message: &str) {
        if self.level < PrintLevel::Normal {
            return;
        }
        let x = format!("\u{283f}[{total}/{total}] {message}: done\n");
        self.print_internal(&x);
    }


    fn take_buffered(&mut self, buf: &mut String) {
        buf.push_str(self.buffered.as_str());
        self.buffered.clear();
    }
}

struct ProgressBarHandle(Arc<Mutex<ProgressBar>>);
impl Drop for ProgressBarHandle {
    fn drop(&mut self) {
        let (total, message) = {
            let Ok(mut bar) = self.0.lock() else {
                return;
            };
            if bar.print_done_when_drop {
                return;
            }
            (bar.total, std::mem::take(&mut bar.message))
        };
        if let Ok(mut x) = GLOBAL_PRINT.lock() {
            x.print_progress_bar_done(total, &message);
        }
    }
}

struct ProgressBar {
    print_done_when_drop: bool,
    total: u64,
    current: u64,
    started: Instant,
    prefix: String,
    message: String,
}

impl ProgressBar {
    /// Format the progress bar, adding at most `width` bytes to the buffer,
    /// not including a newline
    fn format(&self, 
        mut width: usize, 
        now: Instant,
        out: &mut String, temp: &mut String) {
        use std::fmt::Write;
        // format: [current/total] prefix: DD.DD% ETA SS.SSs message
        match width {
            0 => return,
            1 => {
                out.push('.');
                return;
            }
            2 => {
                out.push_str("..");
                return;
            }
            3 => {
                out.push_str("...");
                return;
            }
            4 => {
                out.push_str("[..]");
                return;
            }
            _ => {}
        }
        temp.clear();
        if write!(temp, "{}/{}", self.current, self.total).is_err() {
            temp.clear();
        }
        // .len() is safe because / and numbers have the same byte size and width
        // -2 is safe because width > 4 here
        if temp.len() > width - 2 {
            out.push('[');
            for _ in 0..(width-2) {
                out.push('.');
            }
            out.push(']');
            return;
        }

        width -= 2;
        width -= temp.len();
        out.push_str(temp);
        if width > 0 {
            out.push(' ');
            width -= 1;
        }
        for c in self.prefix.chars() {
            let c_width = c.width_cjk().unwrap_or(0);
            if c_width > width {
                break;
            }
            width -= c_width;
            out.push(c);
        }
        let elapsed = (now - self.started).as_secs_f64();
        // show percentage/ETA if the progress takes more than 2s
        if elapsed > 2f64 && self.current <= self.total{
            // percentage
            // : DD.DD% or : 100%
            if self.current == self.total {
                if width >= 6 {
                    width -= 6;
                    out.push_str(": 100%");
                }
            } else {
                let percentage = self.current as f32 * 100f32 / self.total as f32;
                temp.clear();
                if write!(temp, ": {percentage:.2}%").is_err() {
                    temp.clear();
                }
                if width >= temp.len() {
                    width -= temp.len();
                    out.push_str(temp);
                }
            }
            if width > 0 {
                out.push(' ');
                width -= 1;
            }
            // ETA SS.SSs
            temp.clear();
            let secs_per_unit = elapsed / self.current as f64;
            let eta = secs_per_unit * (self.total - self.current) as f64;
            if write!(temp, "ETA {eta:.2}s").is_err() {
                    temp.clear();
            }
            if width >= temp.len() {
                width -= temp.len();
                out.push_str(temp);
            }
        }
        if width > 0 {
            out.push(' ');
            width -= 1;
        }
        for c in self.message.chars() {
            let c_width = c.width_cjk().unwrap_or(0);
            if c_width > width {
                break;
            }
            width -= c_width;
            out.push(c);
        }
    }
}

fn print_thread(
    original_width: usize,
    first: Arc<Mutex<ProgressBar>>,
    // signal printer that this printing thread is done
    signal: Arc<AtomicBool>,
) -> JoinHandle<()> {
    use std::fmt::Write as _;
    use std::io::Write as _;
    let mut stdout = std::io::stdout();
    std::thread::spawn(move || {
        let max_bars = 4;
        // 50ms between each cycle
        let interval = Duration::from_millis(50);
        let chars = ['\u{280b}', '\u{2819}', '\u{2838}', '\u{2834}', '\u{2826}', '\u{2807}'];

        let mut tick = 0;
        let mut temp = String::new();
        let mut buffer = String::new();
        // how many bars were printed
        let mut lines = 0;
        loop {
            let Ok(mut printer) = GLOBAL_PRINT.lock() else {
                break;
            };
            if printer.prompt_active {
                // don't do anything when there's a prompt,
                // since that will cause cursor to change position
                std::thread::sleep(interval);
                continue;
            }
            let now = Instant::now();
            // remeasure terminal width on every cycle
            let width = terminal_size::terminal_size()
                .map(|(terminal_size::Width(w), _)| w as usize).unwrap_or(original_width) ;

            let mut more_bars = -max_bars;
            buffer.clear();
            buffer += "\r\x1b[K"; // erase the last spacing line (... and X more)
            for _ in 0..lines {
                buffer += "\x1b[1A\x1b[K"; // move up one line and erase it
            }
            if printer.bars.is_empty() {
                break;
            }
            // add the buffered messages
            printer.take_buffered(&mut buffer);
            // print the bars
            buffer += YELLOW;
            lines = 0;
            let anime = chars[tick % 6];
            printer.bars.retain(|bar| {
                let Some(bar) = bar.upgrade() else {
                    return false;
                };
                if more_bars < 0 {
                    // unwrap: when locking the bar for update, it can't panic
                    let bar = bar.lock().unwrap();
                    if width >= 1 {
                        buffer.push(anime);
                        bar.format(width-1, now, &mut buffer, &mut temp);
                    }
                    buffer.push('\n');
                    lines += 1;
                }
                more_bars += 1;

                true
            });

            if more_bars > 0 {
                temp.clear();
                if write!(&mut temp, "  ... and {more_bars} more").is_err() {
                    temp.clear();
                }
                if width >= temp.len() {
                    buffer.push_str(&temp);
                    buffer.push_str(RESET);
                    buffer.push('\r');
                }
            } else {
                buffer.push_str(RESET);
            }


            let _ = write!(stdout, "{buffer}");
            let _ = stdout.flush();


            std::thread::sleep(interval);
            tick+=1;
        }
    })
}
