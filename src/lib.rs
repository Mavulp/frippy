#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

//! Frippy is an IRC bot that runs plugins on each message
//! received.
//!
//! ## Examples
//! ```no_run
//! # extern crate tokio_core;
//! # extern crate futures;
//! # extern crate frippy;
//! # fn main() {
//! use frippy::{plugins, Config, Bot};
//! use tokio_core::reactor::Core;
//! use futures::future;
//!
//! let config = Config::load("config.toml").unwrap();
//! let mut reactor = Core::new().unwrap();
//! let mut bot = Bot::new();
//!
//! bot.add_plugin(plugins::Help::new());
//! bot.add_plugin(plugins::Emoji::new());
//! bot.add_plugin(plugins::Currency::new());
//!
//! bot.connect(&mut reactor, &config);
//! reactor.run(future::empty::<(), ()>()).unwrap();
//! # }
//! ```
//!
//! # Logging
//! Frippy uses the [log](https://docs.rs/log) crate so you can log events
//! which might be of interest.

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate frippy_derive;

extern crate irc;
extern crate futures;
extern crate tokio_core;

pub mod plugin;
pub mod plugins;

use std::fmt;
use std::collections::HashMap;
use std::thread::spawn;
use std::sync::Arc;

use tokio_core::reactor::Core;
pub use irc::client::prelude::*;
pub use irc::error::Error as IrcError;

use plugin::*;

/// The bot which contains the main logic.
#[derive(Default)]
pub struct Bot {
    plugins: ThreadedPlugins,
}

impl Bot {
    /// Creates a `Bot`.
    /// By itself the bot only responds to a few simple ctcp commands
    /// defined per config file.
    /// Any other functionality has to be provided by plugins
    /// which need to implement [`Plugin`](plugin/trait.Plugin.html).
    ///
    /// # Examples
    /// ```
    /// use frippy::Bot;
    /// let mut bot = Bot::new();
    /// ```
    pub fn new() -> Bot {
        Bot { plugins: ThreadedPlugins::new() }
    }

    /// Adds the plugin.
    /// These plugins will be used to evaluate incoming messages from IRC.
    ///
    /// # Examples
    /// ```
    /// use frippy::{plugins, Bot};
    ///
    /// let mut bot = frippy::Bot::new();
    /// bot.add_plugin(plugins::Help::new());
    /// ```
    pub fn add_plugin<T: Plugin + 'static>(&mut self, plugin: T) {
        self.plugins.add(plugin);
    }

    /// Removes a plugin based on its name.
    /// The binary currently uses this to disable plugins
    /// based on user configuration.
    ///
    /// # Examples
    /// ```
    /// use frippy::{plugins, Bot};
    ///
    /// let mut bot = frippy::Bot::new();
    /// bot.add_plugin(plugins::Help::new());
    /// bot.remove_plugin("Help");
    /// ```
    pub fn remove_plugin(&mut self, name: &str) -> Option<()> {
        self.plugins.remove(name)
    }

    /// This connects the `Bot` to IRC and returns a `Future`
    /// which represents the bots work.
    /// This `Future` will run forever unless it returns an error.
    ///
    /// You need to run the `Future`, so that the `Bot`
    /// can actually do its work.
    ///
    /// # Examples
    /// ```no_run
    /// # extern crate tokio_core;
    /// # extern crate futures;
    /// # extern crate frippy;
    /// # fn main() {
    /// use frippy::{Config, Bot};
    /// use tokio_core::reactor::Core;
    /// use futures::future;
    ///
    /// let config = Config::load("config.toml").unwrap();
    /// let mut reactor = Core::new().unwrap();
    /// let mut bot = Bot::new();
    ///
    /// let future = bot.connect(&mut reactor, &config);
    /// reactor.run(future).unwrap();
    /// # }
    /// ```
    pub fn connect(&self, reactor: &mut Core, config: &Config) -> Option<Box<futures::Future<Item = (), Error = ()>>> {
        info!("Plugins loaded: {}", self.plugins);

        let server =
            match IrcServer::new_future(reactor.handle(), config).and_then(|f| {reactor.run(f)}) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    return None;
                }
            };

        info!("Connected to server");

        match server.identify() {
            Ok(_) => info!("Identified"),
            Err(e) => {
                error!("Failed to identify: {}", e);
                return None;
            }
        };

        // TODO Verify if we actually need to clone plugins twice
        let plugins = self.plugins.clone();

        let future = server
            .stream()
            .for_each(move |message| process_msg(&server, plugins.clone(), message))
            .map_err(|e| error!("Failed to process message: {}", e));

        Some(Box::new(future))
    }
}

fn process_msg(server: &IrcServer,
               mut plugins: ThreadedPlugins,
               message: Message)
               -> Result<(), IrcError> {

    // Log any channels we join
    if let Command::JOIN(ref channel, _, _) = message.command {
        if message.source_nickname().unwrap() == server.current_nickname() {
            info!("Joined {}", channel);
        }
    }

    // Check for possible command and save the result for later
    let command = PluginCommand::from(&server.current_nickname().to_lowercase(), &message);

    plugins.execute_plugins(server, message);

    // If the message contained a command, handle it
    if let Some(command) = command {
        if let Err(e) = plugins.handle_command(server, command) {
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
        ThreadedPlugins { plugins: HashMap::new() }
    }

    pub fn add<T: Plugin + 'static>(&mut self, plugin: T) {
        let name = plugin.name().to_lowercase();
        let safe_plugin = Arc::new(plugin);

        self.plugins.insert(name, safe_plugin);
    }

    pub fn remove(&mut self, name: &str) -> Option<()> {
        self.plugins.remove(&name.to_lowercase()).map(|_| ())
    }

    pub fn execute_plugins(&mut self, server: &IrcServer, message: Message) {
        let message = Arc::new(message);

        for (name, plugin) in self.plugins.clone() {
            // Send the message to the plugin if the plugin needs it
            if plugin.is_allowed(server, &message) {

                debug!("Executing {} with {}",
                       name,
                       message.to_string().replace("\r\n", ""));

                // Clone everything before the move - the server uses an Arc internally too
                let plugin = Arc::clone(&plugin);
                let message = Arc::clone(&message);
                let server = server.clone();

                // Execute the plugin in another thread
                spawn(move || {
                          if let Err(e) = plugin.execute(&server, &message) {
                              error!("Error in {} - {}", name, e);
                          };
                      });
            }
        }
    }

    pub fn handle_command(&mut self,
                          server: &IrcServer,
                          mut command: PluginCommand)
                          -> Result<(), IrcError> {

        if !command.tokens.iter().any(|s| !s.is_empty()) {
            let help = format!("Use \"{} help\" to get help", server.current_nickname());
            return server.send_notice(&command.source, &help);
        }

        // Check if the command is for this plugin
        if let Some(plugin) = self.plugins.get(&command.tokens[0].to_lowercase()) {

            // The first token contains the name of the plugin
            let name = command.tokens.remove(0);

            debug!("Sending command \"{:?}\" to {}", command, name);

            // Clone for the move - the server uses an Arc internally
            let server = server.clone();
            let plugin = Arc::clone(plugin);
            spawn(move || {
                      if let Err(e) = plugin.command(&server, command) {
                          error!("Error in {} command - {}", name, e);
                      };
                  });

            Ok(())

        } else {
            let help = format!("\"{} {}\" is not a command, \
                                try \"{0} help\" instead.",
                               server.current_nickname(),
                               command.tokens[0]);

            server.send_notice(&command.source, &help)
        }
    }
}

impl fmt::Display for ThreadedPlugins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let plugin_names = self.plugins
            .iter()
            .map(|(_, p)| p.name().to_string())
            .collect::<Vec<String>>();
        write!(f, "{}", plugin_names.join(", "))
    }
}

#[cfg(test)]
mod tests {}
