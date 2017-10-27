use std::fmt;
use std::collections::HashMap;
use std::thread::spawn;
use std::sync::{Arc, Mutex};

use irc::client::prelude::*;
use irc::error::Error as IrcError;

pub trait Plugin: PluginName + Send + Sync + fmt::Debug {
    fn is_allowed(&self, server: &IrcServer, message: &Message) -> bool;
    fn execute(&mut self, server: &IrcServer, message: &Message) -> Result<(), IrcError>;
    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError>;
}

pub trait PluginName: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
}

#[derive(Clone, Debug)]
pub struct PluginCommand {
    pub source: String,
    pub target: String,
    pub tokens: Vec<String>,
}

impl PluginCommand {
    pub fn from(nick: &str, message: &Message) -> Option<PluginCommand> {

        // Get the actual message out of PRIVMSG
        if let Command::PRIVMSG(_, ref content) = message.command {

            // Split content by spaces and filter empty tokens
            let mut tokens: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            // Commands start with our name
            if tokens[0].to_lowercase().starts_with(nick) {

                // Remove the bot's name from the first token
                tokens[0].drain(..nick.len());

                // We assume that only ':' and ',' are used as suffixes on IRC
                // If there are any other chars we assume that it is not ment for the bot
                tokens[0] = tokens[0]
                    .chars()
                    .filter(|&c| !":,".contains(c))
                    .collect();
                if !tokens[0].is_empty() {
                    return None;
                }

                // The first token contained the name of the bot
                tokens.remove(0);

                Some(PluginCommand {
                         source: message.source_nickname().unwrap().to_string(),
                         target: message.response_target().unwrap().to_string(),
                         tokens: tokens,
                     })
            } else {
                None
            }
        } else {
            None
        }
    }
}

// Lock the mutex and ignore if it is poisoned
macro_rules! lock_plugin {
    ($e:expr) => {
        match $e.lock() {
            Ok(plugin) => plugin,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThreadedPlugins {
    plugins: HashMap<String, Arc<Mutex<Plugin>>>,
}

impl ThreadedPlugins {
    pub fn new() -> ThreadedPlugins {
        ThreadedPlugins { plugins: HashMap::new() }
    }

    pub fn add<T: Plugin + 'static>(&mut self, plugin: T) {
        let name = plugin.name().to_lowercase();
        let safe_plugin = Arc::new(Mutex::new(plugin));

        self.plugins.insert(name, safe_plugin);
    }

    pub fn execute_plugins(&mut self, server: &IrcServer, message: Arc<Message>) {

        for (name, plugin) in self.plugins.clone() {
            // Send the message to the plugin if the plugin needs it
            if lock_plugin!(plugin).is_allowed(server, &message) {

                // Clone everything before the move
                // The server uses an Arc internally too
                let plugin = Arc::clone(&plugin);
                let message = Arc::clone(&message);
                let server = server.clone();

                // Execute the plugin in another thread
                spawn(move || {
                          if let Err(e) = lock_plugin!(plugin).execute(&server, &message) {
                              error!("Error in {} - {}", name, e);
                          };
                      });
            }
        }
    }

    pub fn handle_command(&mut self, server: &IrcServer, mut command: PluginCommand)  -> Result<(), IrcError> {

        if !command.tokens.iter().any(|s| !s.is_empty()) {
            let help = format!("Use \"{} help\" to get help", server.current_nickname());
            return server.send_notice(&command.source, &help);
        }

        if &command.tokens[0].to_lowercase() == "help" {
            return self.send_help_message(server, &command);
        }

        // Check if the command is for this plugin
        if let Some(plugin) = self.plugins.get(&command.tokens[0].to_lowercase()) {

            // The first token contains the name of the plugin
            let name = command.tokens.remove(0);

            // Clone for the move - the server uses an Arc internally
            let server = server.clone();
            let plugin = Arc::clone(plugin);
            spawn(move || {
                      if let Err(e) = lock_plugin!(plugin).command(&server, command) {
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

    fn send_help_message(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source, "Help has not been added yet.")
    }
}

impl fmt::Display for ThreadedPlugins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let plugin_names = self.plugins
            .iter()
            .map(|(_, p)| lock_plugin!(p).name().to_string())
            .collect::<Vec<String>>();
        write!(f, "{}", plugin_names.join(", "))
    }
}
