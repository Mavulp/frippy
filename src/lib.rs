#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate irc;

#[macro_use]
mod plugin;
mod plugins;

use std::thread::spawn;
use std::sync::{Arc, Mutex};
use irc::client::prelude::*;
use irc::proto::Command::PRIVMSG;
use irc::error::Error as IrcError;

use plugin::Plugin;
use plugin::PluginCommand;

pub fn run() {
    let server = IrcServer::new("config.toml").unwrap();
    server.identify().unwrap();

    let plugins: Vec<Arc<Mutex<Plugin>>> =
        vec![Arc::new(Mutex::new(plugins::emoji::Emoji::new())),
             Arc::new(Mutex::new(plugins::currency::Currency::new()))];

    let plugin_names: Vec<String> = plugins
        .iter()
        .map(|p| p.lock().unwrap().to_string().to_lowercase())
        .collect();

    server
        .for_each_incoming(|message| {
            let message = Arc::new(message);
            let command = get_command(&server.current_nickname().to_lowercase(), &message);

            if let Some(ref c) = command {
                if c.tokens.is_empty() {
                    let help = format!("Use \"{} help\" to get help", server.current_nickname());
                    server.send_notice(&c.source, &help).unwrap();

                } else if "help" == &c.tokens[0].to_lowercase() {
                    send_help_message(&server, c).unwrap();

                } else if !plugin_names.contains(&c.tokens[0].to_lowercase()) {

                    let help = format!("\"{} {}\" is not a command, \
                                       try \"{0} help\" instead.",
                                       server.current_nickname(),
                                       c.tokens[0]);

                    server.send_notice(&c.source, &help).unwrap();
                }
            }

            for plugin in plugins.clone() {
                let server = server.clone();
                let message = Arc::clone(&message);
                let command = command.clone();

                spawn(move || {
                    let mut plugin = match plugin.lock() {
                        Ok(plugin) => plugin,
                        Err(poisoned) => poisoned.into_inner(),
                    };

                    if plugin.is_allowed(&server, &message) {
                        plugin.execute(&server, &message).unwrap();
                    }

                    if let Some(mut c) = command {
                        if !c.tokens.is_empty() &&
                           plugin.to_string().to_lowercase() == c.tokens[0].to_lowercase() {

                            c.tokens.remove(0);
                            plugin.command(&server, c).unwrap();
                        }
                    }
                });
            }
        })
        .unwrap();
}

fn send_help_message(server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
    server.send_notice(&command.source, "Help has not been added yet.")
}

fn get_command(nick: &str, message: &Message) -> Option<PluginCommand> {
    if let PRIVMSG(_, ref content) = message.command {
        let mut tokens: Vec<String> = content
            .split(' ')
            .filter(|&x| !x.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if tokens.is_empty() {
            return None;
        }

        if tokens[0].to_lowercase().starts_with(nick) {
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

#[cfg(test)]
mod tests {
    use irc::client::prelude::*;

    pub fn make_server(cmd: &str) -> IrcServer {
        let config = Config {
            nickname: Some(format!("test")),
            server: Some(format!("irc.test.net")),
            channels: Some(vec![format!("#test")]),
            use_mock_connection: Some(true),
            ..Default::default()
        };

        IrcServer::from_config(config).unwrap()
    }

    pub fn get_server_value(server: &IrcServer) -> String {
        unimplemented!();
    }
}
