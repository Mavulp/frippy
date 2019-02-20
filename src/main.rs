#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate frippy;
extern crate glob;
extern crate irc;
extern crate log4rs;
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

use std::collections::HashMap;
#[cfg(feature = "mysql")]
use std::sync::Arc;

use glob::glob;
use irc::client::reactor::IrcReactor;

use frippy::plugins::unicode::Unicode;
use frippy::plugins::factoid::Factoid;
use frippy::plugins::help::Help;
use frippy::plugins::keepnick::KeepNick;
use frippy::plugins::quote::Quote;
use frippy::plugins::remind::Remind;
use frippy::plugins::sed::Sed;
use frippy::plugins::tell::Tell;
use frippy::plugins::url::UrlTitles;

use failure::Error;
use frippy::Config;

#[cfg(feature = "mysql")]
embed_migrations!();

fn main() {
    if let Err(e) = log4rs::init_file("log.yml", Default::default()) {
        use log4rs::Error;
        match e {
            Error::Log(e) => eprintln!("Log4rs error: {}", e),
            Error::Log4rs(e) => eprintln!("Failed to parse \"log.yml\" as log4rs config: {}", e),
        }

        return;
    }

    // Print any errors that caused frippy to shut down
    if let Err(e) = run() {
        let text = e
            .iter_causes()
            .fold(format!("{}", e), |acc, err| format!("{}: {}", acc, err));
        error!("{}", text);
    }
}

fn run() -> Result<(), Error> {
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
        let prefix = prefix.cloned().unwrap_or_else(|| String::from("."));

        let mut bot = frippy::Bot::new(&prefix);
        bot.add_plugin(Help::new());
        bot.add_plugin(UrlTitles::new(1024));
        bot.add_plugin(Sed::new(60));
        bot.add_plugin(Unicode::new());
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
                            bot.add_plugin(Factoid::new(pool.clone()));
                            bot.add_plugin(Quote::new(pool.clone()));
                            bot.add_plugin(Tell::new(pool.clone()));
                            bot.add_plugin(Remind::new(pool.clone()));
                            info!("Connected to MySQL server")
                        }
                        Err(e) => {
                            bot.add_plugin(Factoid::new(HashMap::new()));
                            bot.add_plugin(Quote::new(HashMap::new()));
                            bot.add_plugin(Tell::new(HashMap::new()));
                            bot.add_plugin(Remind::new(HashMap::new()));
                            error!("Failed to run migrations: {}", e);
                        }
                    },
                    Err(e) => error!("Failed to connect to database: {}", e),
                }
            } else {
                bot.add_plugin(Factoid::new(HashMap::new()));
                bot.add_plugin(Quote::new(HashMap::new()));
                bot.add_plugin(Tell::new(HashMap::new()));
                bot.add_plugin(Remind::new(HashMap::new()));
            }
        }
        #[cfg(not(feature = "mysql"))]
        {
            if mysql_url.is_some() {
                error!("frippy was not built with the mysql feature")
            }
            bot.add_plugin(Factoid::new(HashMap::new()));
            bot.add_plugin(Quote::new(HashMap::new()));
            bot.add_plugin(Tell::new(HashMap::new()));
            bot.add_plugin(Remind::new(HashMap::new()));
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
    reactor.run()?;

    Ok(())
}
