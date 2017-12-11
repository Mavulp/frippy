extern crate frippy;
extern crate time;
extern crate tokio_core;
extern crate glob;
extern crate futures;

#[macro_use]
extern crate log;

use log::{LogRecord, LogLevel, LogLevelFilter, LogMetadata};

use tokio_core::reactor::Core;
use futures::future;
use glob::glob;

use frippy::plugins;
use frippy::Config;

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

    // Load all toml files in the configs directory
    let mut configs = Vec::new();
    for toml in glob("configs/*.toml").unwrap() {
        match toml {
            Ok(path) => {
                info!("Loading {}", path.to_str().unwrap());
                match Config::load(path) {
                    Ok(v) => configs.push(v),
                    Err(e) => error!("Incorrect config file {}", e),
                }
            }
            Err(e) => error!("Failed to read path {}", e),
        }
    }

    // Without configs the bot would just idle
    if configs.is_empty() {
        error!("No config file found");
        return;
    }

    // Create an event loop to run the connections on.
    let mut reactor = Core::new().unwrap();

    // Open a connection and add work for each config
    for config in configs {
        let mut bot = frippy::Bot::new();
        bot.add_plugin(plugins::Help::new());
        bot.add_plugin(plugins::Emoji::new());
        bot.add_plugin(plugins::Currency::new());
        bot.add_plugin(plugins::KeepNick::new());

        bot.connect(&mut reactor, &config);
    }

    // Run the main loop forever
    reactor.run(future::empty::<(), ()>()).unwrap();
}
