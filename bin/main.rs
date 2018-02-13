extern crate frippy;
extern crate time;
extern crate tokio_core;
extern crate glob;
extern crate futures;

#[cfg(feature = "mysql")]
#[macro_use]
extern crate diesel_migrations;
#[cfg(feature = "mysql")]
extern crate diesel;

#[macro_use]
extern crate log;

use std::collections::HashMap;

use log::{LogRecord, LogLevel, LogLevelFilter, LogMetadata};

use tokio_core::reactor::Core;
use futures::future;
use glob::glob;

use frippy::plugins;
use frippy::Config;

#[cfg(feature = "mysql")]
embed_migrations!();

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

        let mut disabled_plugins = None;
        let mut mysql_url = None;
        if let &Some(ref options) = &config.options {
            if let Some(disabled) = options.get("disabled_plugins") {
                disabled_plugins = Some(disabled
                                            .split(",")
                                            .map(|p| p.trim())
                                            .collect::<Vec<_>>());
            }

            mysql_url = options.get("mysql_url");
        }

        let mut bot = frippy::Bot::new();
        bot.add_plugin(plugins::Help::new());
        bot.add_plugin(plugins::Url::new(1024));
        bot.add_plugin(plugins::Emoji::new());
        bot.add_plugin(plugins::Currency::new());
        bot.add_plugin(plugins::KeepNick::new());
        #[cfg(feature = "mysql")]
        {
            if let Some(url) = mysql_url {
                use diesel;
                use diesel::Connection;
                match diesel::mysql::MysqlConnection::establish(url) {
                    Ok(conn) => {
                        match embedded_migrations::run(&conn) {
                            Ok(_) => bot.add_plugin(plugins::Factoids::new(conn)),
                            Err(e) => {
                                bot.add_plugin(plugins::Factoids::new(HashMap::new()));
                                error!("Failed to run migrations: {}", e);
                            }
                        }
                    }
                    Err(e) => error!("Failed to connect to database: {}", e),
                }
            } else {
                bot.add_plugin(plugins::Factoids::new(HashMap::new()));
            }
        }
        #[cfg(not(feature = "mysql"))]
        {
            if let Some(_) = mysql_url {
                error!("frippy was not built with the mysql feature")
            }
            bot.add_plugin(plugins::Factoids::new(HashMap::new()));
        }


        if let Some(disabled_plugins) = disabled_plugins {
            for name in disabled_plugins {
                if let None = bot.remove_plugin(name) {
                    error!("\"{}\" was not found - could not disable", name);
                }
            }
        }

        bot.connect(&mut reactor, &config);
    }

    // Run the main loop forever
    reactor.run(future::empty::<(), ()>()).unwrap();
}
