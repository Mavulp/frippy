#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate frippy;
extern crate glob;
extern crate irc;
extern crate time;

#[cfg(feature = "mysql")]
extern crate diesel;
#[cfg(feature = "mysql")]
#[macro_use]
extern crate diesel_migrations;
#[cfg(feature = "mysql")]
extern crate r2d2;
#[cfg(feature = "mysql")]
extern crate r2d2_diesel;

#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;

use log::{Level, LevelFilter, Metadata, Record};
use std::collections::HashMap;
#[cfg(feature = "mysql")]
use std::sync::Arc;

use glob::glob;
use irc::client::reactor::IrcReactor;

use frippy::plugins::currency::Currency;
use frippy::plugins::emoji::Emoji;
use frippy::plugins::factoids::Factoids;
use frippy::plugins::help::Help;
use frippy::plugins::keepnick::KeepNick;
use frippy::plugins::sed::Sed;
use frippy::plugins::tell::Tell;
use frippy::plugins::url::UrlTitles;

use failure::Error;
use frippy::Config;

#[cfg(feature = "mysql")]
embed_migrations!();

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().contains("frippy")
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if record.metadata().level() >= Level::Debug {
                println!(
                    "[{}]({}) {} -> {}",
                    time::now().rfc822(),
                    record.level(),
                    record.target(),
                    record.args()
                );
            } else {
                println!(
                    "[{}]({}) {}",
                    time::now().rfc822(),
                    record.level(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

fn main() {
    // Print any errors that caused frippy to shut down
    if let Err(e) = run() {
        let text = e.causes()
            .skip(1)
            .fold(format!("{}", e), |acc, err| format!("{}: {}", acc, err));
        error!("{}", text);
    };
}

fn run() -> Result<(), Error> {
    log::set_max_level(if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });

    log::set_logger(&LOGGER).unwrap();

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
        bail!("No config file was found");
    }

    // Create an event loop to run the connections on.
    let mut reactor = IrcReactor::new()?;

    // Open a connection and add work for each config
    for config in configs {
        let mut prefix = None;
        let mut disabled_plugins = None;
        let mut mysql_url = None;
        if let Some(ref options) = config.options {
            if let Some(disabled) = options.get("disabled_plugins") {
                disabled_plugins = Some(disabled.split(',').map(|p| p.trim()).collect::<Vec<_>>());
            }
            prefix = options.get("prefix");

            mysql_url = options.get("mysql_url");
        }
        let prefix = prefix.map(|&ref s| s.clone()).unwrap_or(String::from("."));

        let mut bot = frippy::Bot::new(&prefix);
        bot.add_plugin(Help::new());
        bot.add_plugin(UrlTitles::new(1024));
        bot.add_plugin(Sed::new(60));
        bot.add_plugin(Emoji::new());
        bot.add_plugin(Currency::new());
        bot.add_plugin(KeepNick::new());

        #[cfg(feature = "mysql")]
        {
            if let Some(url) = mysql_url {
                use diesel::MysqlConnection;
                use r2d2;
                use r2d2_diesel::ConnectionManager;

                let manager = ConnectionManager::<MysqlConnection>::new(url.clone());
                match r2d2::Pool::builder().build(manager) {
                    Ok(pool) => match embedded_migrations::run(&*pool.get()?) {
                        Ok(_) => {
                            let pool = Arc::new(pool);
                            bot.add_plugin(Factoids::new(pool.clone()));
                            bot.add_plugin(Tell::new(pool.clone()));
                            info!("Connected to MySQL server")
                        }
                        Err(e) => {
                            bot.add_plugin(Factoids::new(HashMap::new()));
                            bot.add_plugin(Tell::new(HashMap::new()));
                            error!("Failed to run migrations: {}", e);
                        }
                    },
                    Err(e) => error!("Failed to connect to database: {}", e),
                }
            } else {
                bot.add_plugin(Factoids::new(HashMap::new()));
                bot.add_plugin(Tell::new(HashMap::new()));
            }
        }
        #[cfg(not(feature = "mysql"))]
        {
            if mysql_url.is_some() {
                error!("frippy was not built with the mysql feature")
            }
            bot.add_plugin(Factoids::new(HashMap::new()));
            bot.add_plugin(Tell::new(HashMap::new()));
        }

        if let Some(disabled_plugins) = disabled_plugins {
            for name in disabled_plugins {
                if bot.remove_plugin(name).is_none() {
                    error!("\"{}\" was not found - could not disable", name);
                }
            }
        }

        bot.connect(&mut reactor, &config)?;
    }

    // Run the bots until they throw an error - an error could be loss of connection
    Ok(reactor.run()?)
}
