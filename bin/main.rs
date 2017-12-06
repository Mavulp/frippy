extern crate frippy;
extern crate log;
extern crate time;

use log::{LogRecord, LogLevel, LogLevelFilter, LogMetadata};

use frippy::plugins;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.target().contains("frippy")
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            if record.metadata().level() >= LogLevel::Debug {
                println!("[{}]({}) {} -> {}",
                         time::now().rfc822(),
                         record.level(),
                         record.target(),
                         record.args());
            } else {
                println!("[{}]({}) {}",
                         time::now().rfc822(),
                         record.level(),
                         record.args());
            }
        }
    }
}

fn main() {

    let log_level = if cfg!(debug_assertions) {
        LogLevelFilter::Debug
    } else {
        LogLevelFilter::Info
    };

    log::set_logger(|max_log_level| {
                        max_log_level.set(log_level);
                        Box::new(Logger)
                    })
            .unwrap();

    let mut bot = frippy::Bot::new();

    bot.add_plugin(plugins::Help::new());
    bot.add_plugin(plugins::Emoji::new());
    bot.add_plugin(plugins::Currency::new());
    bot.add_plugin(plugins::KeepNick::new());

    bot.run();
}
