#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

//! Frippy is an IRC bot that runs plugins on each message
//! received.
//!
//! ## Examples
//! ```no_run
//! use frippy::plugins;
//!
//! let mut bot = frippy::Bot::new();
//!
//! bot.add_plugin(plugins::Help::new());
//! bot.add_plugin(plugins::Emoji::new());
//! bot.add_plugin(plugins::Currency::new());
//!
//! bot.run();
//! ```
//!
//! # Logging
//! Frippy uses the [log](https://docs.rs/log) crate so you can log events
//! which might be of interest.

#[macro_use]
extern crate log;
#[macro_use]
extern crate frippy_derive;

extern crate irc;
extern crate tokio_core;
extern crate futures;
extern crate glob;

pub mod plugin;
pub mod plugins;

use std::fmt;
use std::collections::HashMap;
use std::thread::spawn;
use std::sync::Arc;

use irc::client::prelude::*;
use irc::error::Error as IrcError;

use tokio_core::reactor::Core;
use futures::future;
use glob::glob;

use plugin::*;

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

    /// Add plugins which should evaluate incoming messages from IRC.
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

    /// This starts the `Bot` which means that it tries
    /// to create one connection for each toml file
    /// found in the `configs` directory.
    ///
    /// Then it waits for incoming messages and sends them to the plugins.
    /// This blocks the current thread until the `Bot` is shut down.
    ///
    /// # Examples
    /// ```no_run
    /// use frippy::{plugins, Bot};
    ///
    /// let mut bot = Bot::new();
    /// bot.run();
    /// ```
    pub fn run(self) {
        info!("Plugins loaded: {}", self.plugins);

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
            let server =
                match IrcServer::new_future(reactor.handle(), &config).and_then(|f| {
                                                                                    reactor.run(f)
                                                                                }) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to connect: {}", e);
                        return;
                    }
                };

            info!("Connected to server");

            match server.identify() {
                Ok(_) => info!("Identified"),
                Err(e) => error!("Failed to identify: {}", e),
            };

            // TODO Verify if we actually need to clone plugins twice
            let plugins = self.plugins.clone();

            let task = server
                .stream()
                .for_each(move |message| process_msg(&server, plugins.clone(), message))
                .map_err(|e| error!("Failed to process message: {}", e));

            reactor.handle().spawn(task);
        }

        // Run the main loop forever
        reactor.run(future::empty::<(), ()>()).unwrap();
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

#[derive(Clone, Debug)]
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

    pub fn execute_plugins(&mut self, server: &IrcServer, message: Message) {
        let message = Arc::new(message);

        for (name, plugin) in self.plugins.clone() {
            // Send the message to the plugin if the plugin needs it
            if plugin.is_allowed(server, &message) {

                debug!("Executing {} with {}",
                       name,
                       message.to_string().replace("\r\n", ""));

                // Clone everything before the move
                // The server uses an Arc internally too
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
