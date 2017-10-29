extern crate frippy;
extern crate log;
extern crate time;

use log::{LogRecord, LogLevel, LogLevelFilter, LogMetadata};

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= LogLevel::Info
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            println!("[{}]({}) {}", time::now().rfc822(), record.level(), record.args());
        }
    }
}

fn main() {
    log::set_logger(|max_log_level| {
        max_log_level.set(LogLevelFilter::Info);
        Box::new(Logger)
    }).unwrap();

    frippy::run();
}
