#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

//! Frippy is an IRC bot that runs plugins on each message
//! received.
//!
//! ## Examples
//! ```no_run
//! # extern crate irc;
//! # extern crate frippy;
//! # fn main() {
//! use frippy::{plugins, Config, Bot};
//! use irc::client::reactor::IrcReactor;
//!
//! let config = Config::load("config.toml").unwrap();
//! let mut reactor = IrcReactor::new().unwrap();
//! let mut bot = Bot::new(".");
//!
//! bot.add_plugin(plugins::help::Help::new());
//! bot.add_plugin(plugins::emoji::Emoji::new());
//! bot.add_plugin(plugins::currency::Currency::new());
//!
//! bot.connect(&mut reactor, &config).unwrap();
//! reactor.run().unwrap();
//! # }
//! ```
//!
//! # Logging
//! Frippy uses the [log](https://docs.rs/log) crate so you can log events
//! which might be of interest.

#[cfg(feature = "mysql")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "mysql")]
extern crate r2d2;
#[cfg(feature = "mysql")]
extern crate r2d2_diesel;

#[macro_use]
extern crate failure;
#[macro_use]
extern crate frippy_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

extern crate chrono;
extern crate circular_queue;
extern crate humantime;
extern crate irc;
extern crate regex;
extern crate reqwest;
extern crate time;

pub mod plugin;
pub mod plugins;
pub mod utils;
pub mod error;

use std::collections::HashMap;
use std::fmt;
use std::thread;
use std::sync::Arc;

pub use irc::client::prelude::*;
pub use irc::error::IrcError;
use error::*;
use failure::ResultExt;

use plugin::*;

/// The bot which contains the main logic.
#[derive(Default)]
pub struct Bot<'a> {
    prefix: &'a str,
    plugins: ThreadedPlugins,
}

impl<'a> Bot<'a> {
    /// Creates a `Bot` without any plugins.  
    /// By itself the bot only responds to a few simple CTCP commands
    /// defined per config file.
    /// Any other functionality has to be provided by plugins
    /// which need to implement [`Plugin`](plugin/trait.Plugin.html).  
    /// To send commands to a plugin
    /// the message has to start with the plugin's name prefixed by `cmd_prefix`.
    ///
    /// # Examples
    /// ```
    /// use frippy::Bot;
    /// let mut bot = Bot::new(".");
    /// ```
    pub fn new(cmd_prefix: &'a str) -> Bot<'a> {
        Bot {
            prefix: cmd_prefix,
            plugins: ThreadedPlugins::new(),
        }
    }

    /// Adds the [`Plugin`](plugin/trait.Plugin.html).
    /// These plugins will be used to evaluate incoming messages from IRC.
    ///
    /// # Examples
    /// ```
    /// use frippy::{plugins, Bot};
    ///
    /// let mut bot = frippy::Bot::new(".");
    /// bot.add_plugin(plugins::help::Help::new());
    /// ```
    pub fn add_plugin<T: Plugin + 'static>(&mut self, plugin: T) {
        self.plugins.add(plugin);
    }

    /// Removes a [`Plugin`](plugin/trait.Plugin.html) based on its name.
    /// The binary currently uses this to disable plugins
    /// based on user configuration.
    ///
    /// # Examples
    /// ```
    /// use frippy::{plugins, Bot};
    ///
    /// let mut bot = frippy::Bot::new(".");
    /// bot.add_plugin(plugins::help::Help::new());
    /// bot.remove_plugin("Help");
    /// ```
    pub fn remove_plugin(&mut self, name: &str) -> Option<()> {
        self.plugins.remove(name)
    }

    /// This connects the `Bot` to IRC and creates a task on the
    /// [`IrcReactor`](../irc/client/reactor/struct.IrcReactor.html)
    /// which returns an Ok if the connection was cleanly closed and
    /// an Err if the connection was lost.
    ///
    /// You need to run the [`IrcReactor`](../irc/client/reactor/struct.IrcReactor.html),
    /// so that the `Bot`
    /// can actually do its work.
    ///
    /// # Examples
    /// ```no_run
    /// # extern crate irc;
    /// # extern crate frippy;
    /// # fn main() {
    /// use frippy::{Config, Bot};
    /// use irc::client::reactor::IrcReactor;
    ///
    /// let config = Config::load("config.toml").unwrap();
    /// let mut reactor = IrcReactor::new().unwrap();
    /// let mut bot = Bot::new(".");
    ///
    /// bot.connect(&mut reactor, &config).unwrap();
    /// reactor.run().unwrap();
    /// # }
    /// ```
    pub fn connect(&self, reactor: &mut IrcReactor, config: &Config) -> Result<(), FrippyError> {
        info!("Plugins loaded: {}", self.plugins);

        let client = reactor
            .prepare_client_and_connect(config)
            .context(ErrorKind::Connection)?;

        info!("Connected to IRC server");

        client.identify().context(ErrorKind::Connection)?;
        info!("Identified");

        // TODO Verify if we actually need to clone twice
        let plugins = self.plugins.clone();
        let prefix = self.prefix.to_owned();

        reactor.register_client_with_handler(client, move |client, message| {
            process_msg(client, plugins.clone(), &prefix.clone(), message)
        });

        Ok(())
    }
}

fn process_msg(
    client: &IrcClient,
    mut plugins: ThreadedPlugins,
    prefix: &str,
    message: Message,
) -> Result<(), IrcError> {
    // Log any channels we join
    if let Command::JOIN(ref channel, _, _) = message.command {
        if message.source_nickname().unwrap() == client.current_nickname() {
            info!("Joined {}", channel);
        }
    }

    // Check for possible command and save the result for later
    let command = PluginCommand::try_from(prefix, &message);

    plugins.execute_plugins(client, message);

    // If the message contained a command, handle it
    if let Some(command) = command {
        if let Err(e) = plugins.handle_command(client, command) {
            error!("Failed to handle command: {}", e);
        }
    }

    Ok(())
}

#[derive(Clone, Default, Debug)]
struct ThreadedPlugins {
    plugins: HashMap<String, Arc<Plugin>>,
}

impl ThreadedPlugins {
    pub fn new() -> ThreadedPlugins {
        ThreadedPlugins {
            plugins: HashMap::new(),
        }
    }

    pub fn add<T: Plugin + 'static>(&mut self, plugin: T) {
        let name = plugin.name().to_lowercase();
        let safe_plugin = Arc::new(plugin);

        self.plugins.insert(name, safe_plugin);
    }

    pub fn remove(&mut self, name: &str) -> Option<()> {
        self.plugins.remove(&name.to_lowercase()).map(|_| ())
    }

    /// Runs the execute functions on all plugins.
    /// Any errors that occur are printed right away.
    pub fn execute_plugins(&mut self, client: &IrcClient, message: Message) {
        let message = Arc::new(message);

        for (name, plugin) in self.plugins.clone() {
            // Send the message to the plugin if the plugin needs it
            match plugin.execute(client, &message) {
                ExecutionStatus::Done => (),
                ExecutionStatus::Err(e) => log_error(e),
                ExecutionStatus::RequiresThread => {
                    debug!(
                        "Spawning thread to execute {} with {}",
                        name,
                        message.to_string().replace("\r\n", "")
                    );

                    // Clone everything before the move - the client uses an Arc internally too
                    let plugin = Arc::clone(&plugin);
                    let message = Arc::clone(&message);
                    let client = client.clone();

                    // Execute the plugin in another thread
                    if let Err(e) = thread::Builder::new()
                        .name(name)
                        .spawn(move || {
                            if let Err(e) = plugin.execute_threaded(&client, &message) {
                                log_error(e);
                            };
                        })
                        .context(ErrorKind::ThreadSpawn)
                    {
                        log_error(e.into());
                    }
                }
            }
        }
    }

    pub fn handle_command(
        &mut self,
        client: &IrcClient,
        mut command: PluginCommand,
    ) -> Result<(), FrippyError> {
        // Check if there is a plugin for this command
        if let Some(plugin) = self.plugins.get(&command.tokens[0].to_lowercase()) {
            // The first token contains the name of the plugin
            let name = command.tokens.remove(0);

            debug!("Sending command \"{:?}\" to {}", command, name);

            // Clone for the move - the client uses an Arc internally
            let client = client.clone();
            let plugin = Arc::clone(plugin);
            thread::Builder::new()
                .name(name)
                .spawn(move || {
                    if let Err(e) = plugin.command(&client, command) {
                        log_error(e);
                    };
                })
                .context(ErrorKind::ThreadSpawn)?;
        }

        Ok(())
    }
}

impl fmt::Display for ThreadedPlugins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let plugin_names = self.plugins
            .iter()
            .map(|(_, p)| p.name().to_owned())
            .collect::<Vec<String>>();
        write!(f, "{}", plugin_names.join(", "))
    }
}
