//! The warnings a test's own code writes to the log, read back (BUG-163). Test
//! builds only: the product cannot reach it.
//!
//! Some things the game says only in its log. When a data file in the data
//! folder cannot be read by this version, the loader uses the copy built into
//! the exe and says so in ONE line naming the file and why; before BUG-163 a
//! stale file instead wrote one warning per row and loaded nothing. A test that
//! pins "one clear line, not one per row" has to read the log back, and this is
//! how.
//!
//! `capture` runs a closure and returns, beside its result, every warning and
//! error logged ON THIS THREAD while it ran. Each test runs on a thread of its
//! own, so lines from tests running at the same time never land in the list.
//!
//! The first capture installs this logger for the whole test binary (nothing
//! else in the tests installs one) and lets warnings and errors through. A line
//! logged outside a capture is dropped on the spot. Should a logger ever be
//! installed before this one, nothing is captured and a test counting lines
//! fails loudly rather than passing on an empty list.

use std::cell::RefCell;
use std::sync::Once;

thread_local! {
    /// The lines of the capture running on this thread, if one is.
    static LINES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if record.level() > log::Level::Warn {
            return;
        }
        LINES.with(|lines| {
            if let Some(lines) = lines.borrow_mut().as_mut() {
                lines.push(format!("{}: {}", record.level(), record.args()));
            }
        });
    }

    fn flush(&self) {}
}

static LOGGER: Capture = Capture;
static INSTALL: Once = Once::new();

/// Run `f`, and return what it returned with every warning and error it logged
/// on this thread, each as `LEVEL: message`.
pub(crate) fn capture<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
    INSTALL.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
    LINES.with(|lines| *lines.borrow_mut() = Some(Vec::new()));
    let out = f();
    let lines = LINES.with(|lines| lines.borrow_mut().take()).unwrap_or_default();
    (out, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture sees this thread's warnings and nothing logged before it.
    #[test]
    fn a_capture_holds_the_warnings_written_while_it_ran() {
        log::warn!("before the capture");
        let ((), lines) = capture(|| {
            log::info!("not a warning");
            log::warn!("one warning");
            log::error!("one error");
        });
        assert_eq!(lines, vec!["WARN: one warning".to_string(), "ERROR: one error".to_string()]);
    }
}
