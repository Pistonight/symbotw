use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

pub struct ProgressPrinter {
    term_width: usize,
    total: usize,
    prefix: String,
    thread: Option<JoinHandle<()>>,
    can_print: Arc<AtomicBool>,
    is_done: Arc<AtomicBool>,
    start_time: Instant,
}

impl ProgressPrinter {
    pub fn new(total: usize, prefix: impl Into<String>) -> Self {
        let term_width = if std::io::stderr().is_terminal() {
            match terminal_size::terminal_size() {
                Some((width, _)) => width.0 as usize,
                None => 0,
            }
        } else {
            0
        };

        let prefix = prefix.into();
        let can_print = Arc::new(AtomicBool::new(true));
        let is_done = Arc::new(AtomicBool::new(false));

        // use a thread to throttle the printing
        let thread = {
            let can_print = Arc::clone(&can_print);
            let is_done = Arc::clone(&is_done);
            std::thread::spawn(move || {
                while !is_done.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    can_print.store(true, Ordering::Relaxed);
                }
            })
        };

        Self {
            term_width,
            total,
            prefix,
            thread: Some(thread),
            can_print,
            is_done,
            start_time: Instant::now(),
        }
    }

    pub fn print(&self, current: usize, text: impl std::fmt::Display) {
        if !self.can_print.load(Ordering::Relaxed) {
            return;
        }
        self.can_print.store(false, Ordering::Relaxed);
        let prefix = if self.total == 0 {
            format!("{1} {0} ", self.prefix, current)
        } else {
            let mut s = format!("[{1}/{2}] {0}: ", self.prefix, current, self.total);
            let elapsed = self.start_time.elapsed().as_secs_f32();
            if elapsed > 2.0 {
                let percentage = format!("{:.02}% ", (current as f32 / self.total as f32) * 100.0);
                let speed = current as f32 / elapsed;
                let eta = format!("ETA {:.02}s ", (self.total - current) as f32 / speed);
                s.push_str(&percentage);
                s.push_str(&eta);
            }
            s
        };
        let prefix_len = prefix.len();
        let prefix = if prefix_len > self.term_width {
            &prefix[prefix_len - self.term_width + 1..]
        } else {
            &prefix
        };
        let remaining = self.term_width - prefix_len - 1;
        let text = text.to_string();
        let text_len = text.len();
        let text = if text_len > remaining {
            let start = text_len - remaining;
            &text[start..]
        } else {
            &text
        };
        eprint!("\u{1b}[1K\r{}{}", prefix, text);
        let _ = std::io::stderr().flush();
    }

    pub fn set_total(&mut self, total: usize) {
        self.total = total;
    }

    pub fn reset_timer(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
    }

    pub fn done(&self) {
        self.is_done.store(true, Ordering::Relaxed);
        if self.total == 0 {
            println!("\u{1b}[1K\r{}", self.prefix);
        } else {
            println!("\u{1b}[1K\r[{1}/{1}] {0}", self.prefix, self.total);
        }
    }
}

impl Drop for ProgressPrinter {
    fn drop(&mut self) {
        self.is_done.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
