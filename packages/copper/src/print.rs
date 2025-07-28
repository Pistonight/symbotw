use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use clap::ValueEnum;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorLevel {
    Always,
    Never,
    #[default]
    Auto,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PrintLevel {
    QuietQuiet,
    Quiet,
    Normal,
    Verbose,
    VerboseVerbose
}
impl From<i8> for PrintLevel {
    fn from(value: i8) -> Self {
        match value {
            ..=-2 => Self::QuietQuiet,
            -1 => Self::Quiet,
            0 => Self::Normal,
            1 => Self::Verbose,
            2.. => Self::VerboseVerbose
        }
    }
}
impl From<PrintLevel> for log::LevelFilter {
    fn from(value: PrintLevel) -> Self {
        match value {
            PrintLevel::QuietQuiet => log::LevelFilter::Off,
            PrintLevel::Quiet => log::LevelFilter::Error,
            PrintLevel::Normal => log::LevelFilter::Info,
            PrintLevel::Verbose => log::LevelFilter::Debug,
            PrintLevel::VerboseVerbose => log::LevelFilter::Trace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PromptLevel {
    /// Show prompts interactively
    Interactive,
    /// Automatically answer "Yes" to all yes/no prompts, and `Auto` for regular prompts
    Yes,
    /// Do not allow prompts (non-interactive). Attempting to show prompt will error
    No
}

#[derive(Debug, Clone, Copy)]
pub enum __PrintType {
    Error,
    Hint,
    Print,
    Warn,
    Info,
    Debug,
    Trace,
}
impl __PrintType {
    fn can_print(self, level: PrintLevel) -> bool {
        match self {
            __PrintType::Error |
            __PrintType::Hint |
            __PrintType::Print 
            => level != PrintLevel::QuietQuiet,
            __PrintType::Warn |
            __PrintType::Info => level > PrintLevel::Quiet,
            __PrintType::Debug => level > PrintLevel::Normal,
            __PrintType::Trace => level == PrintLevel::VerboseVerbose
        }
    }
}
impl From<log::Level> for __PrintType {
    fn from(value: log::Level) -> Self {
        match value {
            log::Level::Error => Self::Error,
            log::Level::Warn => Self::Warn,
            log::Level::Info => Self::Info,
            log::Level::Debug => Self::Debug,
            log::Level::Trace => Self::Trace,
        }
    }
}

static GLOBAL_LOG_FILTER: OnceLock<env_filter::Filter> = OnceLock::new();
/// Set the global log filter
pub(crate) fn set_log_filter(filter: env_filter::Filter) {
    let _ = GLOBAL_LOG_FILTER.set(filter);
}
static GLOBAL_PROMPT_LEVEL: AtomicU8 = AtomicU8::new(0);
fn get_prompt_level() -> PromptLevel {
    let v = GLOBAL_PROMPT_LEVEL.load(Ordering::SeqCst);
    unsafe { std::mem::transmute(v) }
}
fn set_prompt_level(level: PromptLevel) {
    GLOBAL_PROMPT_LEVEL.store(level as u8, Ordering::SeqCst);
}

static GLOBAL_PRINT_LEVEL: AtomicU8 = AtomicU8::new(2);
fn get_print_level() -> PrintLevel {
    let v = GLOBAL_PRINT_LEVEL.load(Ordering::SeqCst);
    unsafe { std::mem::transmute(v) }
}
fn set_print_level(level: PrintLevel) {
    GLOBAL_PRINT_LEVEL.store(level as u8, Ordering::SeqCst);
}
static GLOBAL_USE_COLOR: AtomicBool = AtomicBool::new(true);
static GLOBAL_PRINT: LazyLock<Mutex<Printer>> = LazyLock::new(|| Mutex::new(Printer::default()));

/// Set global print options. This is usually called from clap args
///
/// If prompt option is `None`, it will be `Interactive` unless env var `CI` is `true` or `1`, in which case it becomes `No`
pub fn init_print_options(color: ColorLevel, level: PrintLevel, prompt: Option<PromptLevel>) {
    use std::io::IsTerminal;

    let log_level = if let Ok(value) = std::env::var("RUST_LOG") && !value.is_empty() {
        let mut builder = env_filter::Builder::new();
        let filter = builder.parse(&value).build();
        let log_level = filter.filter();
        set_log_filter(filter);
        log_level.max(level.into())
    } else {
        level.into()
    };
    log::set_max_level(log_level);
    let use_color = match color {
        ColorLevel::Always => true,
        ColorLevel::Never => false,
        ColorLevel::Auto => std::io::stdout().is_terminal()
    };
    GLOBAL_USE_COLOR.store(use_color, Ordering::SeqCst);
    if let Ok(mut printer) = GLOBAL_PRINT.lock() {
        printer.colors = if use_color { COLOR } else { NOCOLOR };
    }
    set_print_level(level);

    struct LogImpl;
    impl log::Log for LogImpl {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            match GLOBAL_LOG_FILTER.get() {
                Some(filter) => {
                    let x = filter.enabled(metadata);
                    // eprintln!("{filter:?}, enabled={x}");
                    return x
                }
                None => {
                    let typ: __PrintType = metadata.level().into();
                    typ.can_print(get_print_level())
                }
            }
        }

        fn log(&self, record: &log::Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let typ: __PrintType = record.level().into();
            let message = record.args().to_string();
            if let Ok(mut printer) = GLOBAL_PRINT.lock() {
                printer.format_message(typ, &message);
                printer.print_format_buffer();
            }
        }

        fn flush(&self) { }
    }

    let _ = log::set_logger(&LogImpl);

    let prompt = match prompt {
        Some(x) => x,
        None => {
            let is_ci = std::env::var("CI").map(|mut x| {
                x.make_ascii_lowercase();
                matches!(x.trim(), "true" | "1")
            }).unwrap_or_default();
            if is_ci {
                PromptLevel::No
            } else {
                PromptLevel::Interactive
            }
        }
    };
    set_prompt_level(prompt);
}


/// Get if color printing is enabled
pub fn color_enabled() -> bool {
    GLOBAL_USE_COLOR.load(Ordering::SeqCst)
}

thread_local! {
    static THREAD_NAME: RefCell<Option<String>> = RefCell::new(None);
}

/// Set the name to show up in messages printed by the current thread
pub fn set_thread_print_name(name: &str) {
    THREAD_NAME.with_borrow_mut(|x| *x = Some(name.to_string()))
}

/// Internal print function for macros
pub fn __print_with_type(typ: __PrintType, message: std::fmt::Arguments<'_>) {
    if typ.can_print(get_print_level()) {
        let message = format!("{message}");
        if let Ok(mut printer) = GLOBAL_PRINT.lock() {
            printer.format_message(typ, &message);
            printer.print_format_buffer();
        }
    }
}

pub fn __prompt_yesno(message: std::fmt::Arguments<'_>) -> crate::Result<bool> {
    match get_prompt_level() {
        PromptLevel::Interactive => {}
        PromptLevel::Yes => return Ok(true),
        PromptLevel::No => {
            crate::bailand!(error!("prompt not allowed in non-interactive mode: {message}"));
        }
    }

    let message = format!("{message} [y/n]");
    let _scope = PromptJoinScope;
    loop {
        let recv = {
            let Ok(mut printer) = GLOBAL_PRINT.lock() else {
                crate::bailand!(error!("prompt failed: global print lock poisoned"));
            };
            printer.format_prompt(&message);
            printer.prompt_format_buffer()
        };
        let result = recv.recv().with_context(|| format!("recv error while showing the prompt: {message}"))?;
        match result {
            Err(e) => {
                Err(e).context(format!("io error while showing the prompt: {message}"))?;
            }
            Ok(mut x) => {
                x.make_ascii_lowercase();
                match x.trim() {
                    "y" | "yes" => return Ok(true),
                    "n" | "no" => return Ok(false),
                    _ => {}
                }
            }
        }
        crate::error!("please enter yes or no");
    }
    
}

pub fn __prompt(message: std::fmt::Arguments<'_>) -> crate::Result<String> {
    if let PromptLevel::No = get_prompt_level() {
        crate::bailand!(error!("prompt not allowed in non-interactive mode: {message}"));
    }
    let message = format!("{message}");
    let _scope = PromptJoinScope;
    let recv = {
        let Ok(mut printer) = GLOBAL_PRINT.lock() else {
            crate::bailand!(error!("prompt failed: global print lock poisoned"));
        };
        printer.format_prompt(&message);
        printer.prompt_format_buffer()
    };
    let result = recv.recv().with_context(|| format!("recv error while showing the prompt: {message}"))?;

    result.with_context(|| format!("io error while showing the prompt: {message}"))
}



/// Print something
#[macro_export]
macro_rules! print {
    ($($fmt_args:tt)*) => {{
        $crate::__priv::__print_with_type($crate::__priv::__PrintType::Print, format_args!($($fmt_args)*));
    }}
}
#[macro_export]
macro_rules! hint {
    ($($fmt_args:tt)*) => {{
        $crate::__priv::__print_with_type($crate::__priv::__PrintType::Hint, format_args!($($fmt_args)*));
    }}
}
/// Show a Yes/No prompt
#[cfg(feature = "prompt")]
#[macro_export]
macro_rules! yesno {
    ($($fmt_args:tt)*) => {{
        $crate::__priv::__prompt_yesno(format_args!($($fmt_args)*))
    }}
}
/// Show a prompt
#[cfg(feature = "prompt")]
#[macro_export]
macro_rules! prompt {
    ($($fmt_args:tt)*) => {{
        $crate::__priv::__prompt(format_args!($($fmt_args)*))
    }}
}
/// Update a progress bar
#[macro_export]
macro_rules! progress {
    ($bar:ident, $current:expr) => {
        $bar.set($current, None);
    };
    ($bar:ident, $current:expr, $($fmt_args:tt)*) => {{
        let message = format!($($fmt_args)*);
        $bar.set($current, Some(message));
    }}
}
/// Format and invoke a print macro
///
/// # Example
/// ```rust
///
/// let x = cu::fmtand!(error!("found {} errors", 3));
/// assert_eq!(x, "found 3 errors");
/// ```
#[macro_export]
macro_rules! fmtand {
    ($mac:ident !( $($fmt_args:tt)* )) => {{
        let s = format!($($fmt_args)*);
        $crate::$mac!("{s}");
        s
    }}
}
/// Invoke a print macro, and bail with the same message
///
/// # Example
/// ```rust,no_run
///
/// let x = cu::bailand!(error!("found {} errors", 3));
/// ```
#[macro_export]
macro_rules! bailand {
    ($mac:ident !( $($fmt_args:tt)* )) => {{
        let s = format!($($fmt_args)*);
        $crate::$mac!("{s}");
        $crate::bail!(s);
    }}
}

pub fn progress_bar(total: usize, message: impl Into<String>) -> ProgressBarHandle {
    let bar = Arc::new(Mutex::new(ProgressBar::new(total, message.into(), true)));
    if let Ok(mut printer) = GLOBAL_PRINT.lock() {
        printer.add_progress_bar(Arc::clone(&bar));
    }
    ProgressBarHandle::new(bar)
}
pub fn progress_bar_lowp(total: usize, message: impl Into<String>) -> ProgressBarHandle {
    let bar = Arc::new(Mutex::new(ProgressBar::new(total, message.into(), false)));
    if let Ok(mut printer) = GLOBAL_PRINT.lock() {
        printer.add_progress_bar(Arc::clone(&bar));
    }
    ProgressBarHandle::new(bar)
}

#[derive(Clone, Copy)]
struct Colors {
    reset: &'static str,
    yellow: &'static str,
    red: &'static str,
    gray: &'static str,
    magenta: &'static str,
    cyan: &'static str,
    green: &'static str,
}

#[derive(Clone, Copy)]
struct Controls {
    move_to_begin_and_clear: &'static str,
}

static NOCOLOR: Colors = Colors {
    reset: "",
    yellow: "",
    red: "",
    gray: "",
    magenta: "",
    cyan: "",
    green: "",
};

static COLOR: Colors = Colors {
    reset: "\x1b[0m",
    yellow: "\x1b[1;33m",
    red: "\x1b[1;31m",
    gray: "\x1b[1;30m",
    magenta: "\x1b[1;35m",
    cyan: "\x1b[1;36m",
    green: "\x1b[1;32m",
};

static NOCONTROL: Controls = Controls {
    move_to_begin_and_clear: ""
};

static CONTROL: Controls = Controls {
    move_to_begin_and_clear: "\r\x1b[K"
};


#[derive(PartialEq, Eq)]
enum ProgressBarTarget {
    /// Don't print progress bars at all, when output is not terminal
    None,
    /// Print to Stdout if Stdout is terminal
    Stdout,
    /// Print to Stderr if Stdout is not terminal and Stderr is
    Stderr,
}

impl Default for ProgressBarTarget {
    fn default() -> Self {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() {
            return Self::Stdout;
        }
        if std::io::stderr().is_terminal() {
            return Self::Stderr;
        }
        Self::None
    }
}

pub struct Printer {
    stdout: std::io::Stdout,
    colors: Colors,
    controls: Controls,

    bar_target: ProgressBarTarget,
    print_thread_stopped: Arc<AtomicBool>,
    print_thread_handle: Option<JoinHandle<()>>,
    bars: Vec<Weak<Mutex<ProgressBar>>>,

    prompt_active: bool,
    pending_prompts: VecDeque<(oneshot::Sender<std::io::Result<String>>, String)>,
    prompt_thread_handle: Option<JoinHandle<()>>,

    format_buffer: FormatBuffer,
    buffered: String,
}

impl Default for Printer {
    fn default() -> Self {
        use std::io::IsTerminal as _;
        let is_terminal = std::io::stdout().is_terminal();
        Self {
            stdout: std::io::stdout(),
            colors: if is_terminal { COLOR } else { NOCOLOR },
            controls: if is_terminal { CONTROL } else { NOCONTROL },

            bar_target: ProgressBarTarget::default(),
            print_thread_stopped: Arc::new(AtomicBool::new(true)),
            print_thread_handle: None,
            bars: Default::default(),

            prompt_active: false,
            pending_prompts: Default::default(),
            prompt_thread_handle: None,

            format_buffer: FormatBuffer::new(),
            buffered: String::new(),
        }
    }
}

impl Printer {
    fn prompt_format_buffer(&mut self) -> oneshot::Receiver<std::io::Result<String>> {
        // x is already formatted
        let (send, recv) = oneshot::channel();
        if !self.prompt_active {
            if let Some(x) = self.prompt_thread_handle.take() {
                let _ = x.join();
            }
            use std::io::Write;
            self.prompt_active = true;
            // erase current line, and print new prompt
            // this may mess up progress bars - having both prompts
            // and progress bar is not a good idea anyway
            let _ = write!(self.stdout, "{}{}{}", self.controls.move_to_begin_and_clear, self.buffered, self.format_buffer.as_str());
            self.buffered.clear();
            let _ = self.stdout.flush();
            self.prompt_thread_handle = Some(prompt_thread(send));
            return recv;
        }
        self.pending_prompts.push_back((send, self.format_buffer.take()));
        return recv;
    }

    fn on_prompt_done(&mut self) -> Option<JoinHandle<()>> {
        if self.prompt_thread_handle.is_none() {
            return None;
        }
        if self.pending_prompts.is_empty() {
            self.prompt_thread_handle.take()
        } else {
            None
        }
    }


    fn print_format_buffer(&mut self) {
        if !self.prompt_active && self.bars.is_empty() {
            use std::io::Write;
            // let _ = write!(self.stdout, "{}", std::mem::take(&mut self.buffered));
            let _ = write!(self.stdout, "{}", self.format_buffer.as_str());
            let _ = self.stdout.flush();
            
            return;
        }
        self.buffered.push_str(self.format_buffer.as_str());

    }

    fn add_progress_bar(&mut self, bar: Arc<Mutex<ProgressBar>>) {
        if get_print_level() < PrintLevel::Normal {
            return;
        }
        if self.bar_target == ProgressBarTarget::None {
            return;
        }
        if self.print_thread_stopped.load(Ordering::SeqCst) {
            // don't use bar if we can't measure terminal size
            let Some(width) = term_width() else {
                return;
            };
            // spawn new printing thread
            let new_signal = Arc::new(AtomicBool::new(false));
            let new_signal2 = Arc::clone(&new_signal);
            self.print_thread_stopped = new_signal;
            if let Some(handle) = self.print_thread_handle.take() {
                let _ = handle.join();
            }
            self.print_thread_handle = Some(print_thread(width, new_signal2));
        }
        self.bars.push(Arc::downgrade(&bar));
    }

    fn on_progress_bar_done(&mut self) -> Option<JoinHandle<()>> {
        if self.print_thread_handle.is_none() {
            return None;
        }
        let strong_count = self.bars.iter().filter(|x| x.upgrade().is_some()).count();
        if strong_count == 0 {
            self.print_thread_handle.take()
        } else {
            None
        }
    }

    fn print_progress_bar_done(&mut self, total: usize, message: &str) {
        if get_print_level() < PrintLevel::Normal {
            return;
        }
        self.format_buffer.reset(self.colors.gray, self.colors.green);
        self.format_buffer.push_control(self.colors.green);
        self.format_buffer.push_str(&format!("\u{283f}][{total}/{total}] {message}: done"));
        self.format_buffer.end();
        self.print_format_buffer();
    }


    fn take_buffered(&mut self, buf: &mut String) {
        buf.push_str(self.buffered.as_str());
        self.buffered.clear();
    }

    /// format the prompt into the printer's format buffer
    fn format_prompt(&mut self, prompt: &str) {
        let mut lines = prompt.lines();
        self.format_buffer.reset(self.colors.gray, self.colors.cyan);
        self.format_buffer.push_control(self.colors.cyan);
        self.format_buffer.push('!', 1);
        self.format_buffer.push(']', 1);
        if let Some(line) = lines.next() {
            self.format_buffer.push(' ', 1);
            self.format_buffer.push_str(line);
        }
        for line in lines {
            self.format_buffer.new_line();
            self.format_buffer.push_str(line);
        }
        self.format_buffer.end();
        self.format_buffer.push_control(self.colors.reset);
        self.format_buffer.push_control("-: ");
    }

    /// format the message into the printer's format buffer
    fn format_message(&mut self, typ: __PrintType, message: &str) {
        let mut lines = message.lines();
        let text_color = match typ {
            __PrintType::Error => self.colors.red,
            __PrintType::Hint => self.colors.yellow,
            __PrintType::Print => self.colors.reset,
            __PrintType::Warn => self.colors.yellow,
            __PrintType::Info => self.colors.reset,
            __PrintType::Debug => self.colors.gray,
            __PrintType::Trace => self.colors.magenta,
        };
        self.format_buffer.reset(self.colors.gray, text_color);
        match typ {
            __PrintType::Error => {
                self.format_buffer.push_control(self.colors.red);
                self.format_buffer.push('E', 1);
                self.format_buffer.push(']', 1);
            }
            __PrintType::Hint => {
                self.format_buffer.push_control(self.colors.cyan);
                self.format_buffer.push('H', 1);
                self.format_buffer.push_control(self.colors.gray);
                self.format_buffer.push(']', 1);
            }
            __PrintType::Print => {
                self.format_buffer.push_control(self.colors.gray);
                self.format_buffer.push(' ', 1);
                self.format_buffer.push(':', 1);
            }
            __PrintType::Warn => {
                self.format_buffer.push_control(self.colors.yellow);
                self.format_buffer.push('W', 1);
                self.format_buffer.push(']', 1);
            }
            __PrintType::Info => {
                self.format_buffer.push_control(self.colors.green);
                self.format_buffer.push('I', 1);
                self.format_buffer.push_control(self.colors.gray);
                self.format_buffer.push(']', 1);
            }
            __PrintType::Debug => {
                self.format_buffer.push_control(self.colors.gray);
                self.format_buffer.push('D', 1);
                self.format_buffer.push(']', 1);
            }
            __PrintType::Trace => {
                self.format_buffer.push_control(self.colors.magenta);
                self.format_buffer.push('*', 1);
                self.format_buffer.push(']', 1);
            }
        }
        THREAD_NAME.with_borrow(|x| {
            if let Some(x) = x {
                self.format_buffer.push_control(self.colors.magenta);
                self.format_buffer.push('[', 1);
                self.format_buffer.push_str(x);
                self.format_buffer.push(']', 1);
            }
        });
        self.format_buffer.push_control(text_color);
        if let Some(line) = lines.next() {
            self.format_buffer.push(' ', 1);
            self.format_buffer.push_str(line);
        }
        for line in lines {
            self.format_buffer.new_line();
            self.format_buffer.push_str(line);
        }
        self.format_buffer.end();
    }
}

pub struct ProgressBarHandle(std::mem::ManuallyDrop<Arc<Mutex<ProgressBar>>>);
impl Drop for ProgressBarHandle {
    fn drop(&mut self) {
        let (print_done_when_drop, total, message) = {
            match self.0.lock() {
                Ok(mut bar) => (bar.print_done_when_drop, bar.total, std::mem::take(&mut bar. prefix)),
                Err(_) => (false, 0, String::new())
            }
        };
        unsafe { std::mem::ManuallyDrop::drop(&mut self.0) };
        let handle = if let Ok(mut x) = GLOBAL_PRINT.lock() {
            if print_done_when_drop {
                x.print_progress_bar_done(total, &message);
            }
            x.on_progress_bar_done()
        } else {
            None
        };
        if let Some(x) = handle {
            let _ = x.join();
        }
    }
}
impl ProgressBarHandle {
    fn new(bar: Arc<Mutex<ProgressBar>>) -> Self {
        Self(std::mem::ManuallyDrop::new(bar))
    }
    pub fn set(&self, current: usize, message: Option<String>) {
        if let Ok(mut bar) = self.0.lock() {
            bar.current = current;
            if let Some(x) = message {
                bar.message = x;
            }
        }
    }
}

struct ProgressBar {
    print_done_when_drop: bool,
    total: usize,
    current: usize,
    started: Instant,
    prefix: String,
    message: String,
}

impl ProgressBar {
    fn new(total: usize, prefix: String, print_done_when_drop: bool) -> Self {
        Self {
            print_done_when_drop,
            total,
            current: 0,
            started: Instant::now(),
            prefix,
            message: String::new()
        }
    }
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
        out.push('[');
        out.push_str(temp);
        out.push(']');
        if width > 0 {
            out.push(' ');
            width -= 1;
        }
        for (c, w) in with_ansi_width(self.prefix.chars()) {
            if w > width {
                break;
            }
            width -= w;
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
        for (c, w) in with_ansi_width(self.message.chars()) {
            if w > width {
                break;
            }
            width -= w;
            out.push(c);
        }
    }
}

fn print_thread(original_width: usize, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    use std::fmt::Write as _;
    use std::io::Write as _;
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
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
            if stop.load(Ordering::SeqCst) {
                break;
            }
            buffer.clear();
            buffer += "\r\x1b[K"; // erase the last spacing line (... and X more)
            for _ in 0..lines {
                buffer += "\x1b[1A\x1b[K"; // move up one line and erase it
            }
            {
                let Ok(mut printer) = GLOBAL_PRINT.lock() else {
                    stop.store(true, Ordering::SeqCst);
                    break;
                };
                if printer.bar_target == ProgressBarTarget::None {
                    stop.store(true, Ordering::SeqCst);
                    break;
                }
                if printer.prompt_active {
                    // don't do anything when there's a prompt,
                    // since that will cause cursor to change position
                    std::thread::sleep(interval);
                    continue;
                }
                let now = Instant::now();
                // remeasure terminal width on every cycle
                let width = term_width().unwrap_or(original_width) ;

                let mut more_bars = -max_bars;
                if printer.bar_target == ProgressBarTarget::Stdout {
                    // add the buffered messages
                    printer.take_buffered(&mut buffer);
                } else {
                    let _ = write!(stdout, "{}", printer.buffered);
                    printer.buffered.clear();
                    let _ = stdout.flush();
                }
                // print the bars
                buffer += printer.colors.yellow;
                lines = 0;
                let anime = chars[tick % 6];
                printer.bars.retain(|bar| {
                    let Some(bar) = bar.upgrade() else {
                        return false;
                    };
                    if more_bars < 0 {
                        // unwrap: when locking the bar for update, it can't panic
                        let bar = bar.lock().unwrap();
                        if width >= 2 {
                            buffer.push(anime);
                            buffer.push(']');
                            bar.format(width-2, now, &mut buffer, &mut temp);
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
                        buffer.push_str(printer.colors.reset);
                        buffer.push('\r');
                    }
                } else {
                    buffer.push_str(printer.colors.reset);
                }

                match printer.bar_target  {
                    ProgressBarTarget::None => {},
                    ProgressBarTarget::Stdout => {
                        let _ = write!(stdout, "{buffer}");
                        let _ = stdout.flush();
                    },
                    ProgressBarTarget::Stderr => {
                        let _ = write!(stderr, "{buffer}");
                        let _ = stderr.flush();
                    }
                }

                if printer.bars.is_empty() {
                    // erase the bars
                    buffer.clear();
                    buffer += "\r\x1b[K"; // erase the last spacing line (... and X more)
                    for _ in 0..lines {
                        buffer += "\x1b[1A\x1b[K"; // move up one line and erase it
                    }
                    match printer.bar_target  {
                        ProgressBarTarget::None => {},
                        ProgressBarTarget::Stdout => {
                            printer.take_buffered(&mut buffer);
                            let _ = write!(stdout, "{buffer}");
                            let _ = stdout.flush();
                        },
                        ProgressBarTarget::Stderr => {
                            let _ = write!(stdout, "{}", printer.buffered);
                            printer.buffered.clear();
                            let _ = stdout.flush();
                            let _ = write!(stderr, "{buffer}");
                            let _ = stderr.flush();
                        }
                    }
                    stop.store(true, Ordering::SeqCst);
                    break;
                }
            }

            std::thread::sleep(interval);
            tick+=1;
        }
    })
}

struct PromptJoinScope;
impl Drop for PromptJoinScope {
    fn drop(&mut self) {
        let handle = {
            let Ok(mut printer) = GLOBAL_PRINT.lock() else {
                return;
            };
            let Some(handle) = printer.on_prompt_done() else {
                return;
            };
            handle
        };
        let _ = handle.join();
    }
}

fn prompt_thread(first_send: oneshot::Sender<std::io::Result<String>>) -> JoinHandle<()> {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    std::thread::spawn(move || {
        let mut send = first_send;
        let mut buf = String::new();
        loop {
            buf.clear();
            let result = std::io::stdin().read_line(&mut buf);
            let _ = send.send(result.map(|_| buf.clone()));
            let Ok(mut printer) = GLOBAL_PRINT.lock() else {
                break;
            };
            let Some(next) = printer.pending_prompts.pop_front() else {
                printer.prompt_active = false;
                break;
            };
            let _ = write!(stdout, "{}{}{}", printer.controls.move_to_begin_and_clear, printer.buffered, next.1);
            printer.buffered.clear();
            let _ = stdout.flush();
            send = next.0;
        }
    })
}

struct FormatBuffer {
    width: usize,
    curr: usize,
    buffer: String,
    gray_color: &'static str,
    text_color: &'static str,
}

impl FormatBuffer {
    pub fn new() -> Self {
        Self {
            width: term_width_or_max(),
            curr: 0,
            buffer: String::new(),
            gray_color: "",
            text_color: "",
        }
    }
    pub fn as_str(&self) -> &str {
        self.buffer.as_str()
    }
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }
    pub fn reset(&mut self, gray_color: &'static str, text_color: &'static str) {
        self.curr = 0;
        self.buffer.clear();
        self.width = term_width_or_max();
        self.gray_color = gray_color;
        self.text_color = text_color;
    }
    pub fn end(&mut self) {
        self.buffer.push('\n');
    }

    pub fn push_str(&mut self, x: &str) {
        for (c, w) in with_ansi_width(x.chars()) {
            self.push(c, w);
        }
    }
    pub fn push_control(&mut self, x: &str) {
        self.buffer.push_str(x)
    }
    pub fn push(&mut self, c: char, w: usize) {
        if c == '\n' {
            self.new_line();
            return;
        }
        if self.width < 5 {
            // give up
            self.buffer.push(c);
            return;
        }
        if w < self.width && self.curr > self.width - w {
            self.new_line();
        }
        self.buffer.push(c);
        self.curr += w;
    }

    fn new_line(&mut self) {
        self.buffer.push('\n');
        self.buffer.push_str(self.gray_color);
        self.buffer.push_str(" | ");
        self.buffer.push_str(self.text_color);
        self.curr = 3;
    }
}

fn term_width_or_max() -> usize {
    term_width().unwrap_or(400)
}

fn term_width() -> Option<usize> {
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| (w as usize).min(400))
}

fn with_ansi_width(x: std::str::Chars<'_>) -> AnsiWidthIter<'_> {
    AnsiWidthIter { is_escaping: false, chars: x }
}

struct AnsiWidthIter<'a> {
    is_escaping: bool,
    chars: std::str::Chars<'a>,
}

impl<'a> Iterator for AnsiWidthIter<'a> {
    type Item = (char, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let c = self.chars.next()?;
        // we only do very basic check right now
        let width = if self.is_escaping {
            if c < u8::MAX as char && b"mAKGJBCDEFHSTfhlin".into_iter().find(|x|**x==c as u8).is_some() {
                self.is_escaping = false;
            }
            0
        } else {
            if c == '\x1b' {
                self.is_escaping = true;
                0
            } else {
                use unicode_width::UnicodeWidthChar;
                c.width_cjk().unwrap_or(0)
            }
        };

        Some((c, width))
    }
}
