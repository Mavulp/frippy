use irc::client::prelude::*;
use irc::error::IrcError;

use plugin::*;

#[derive(PluginName, Default, Debug)]
pub struct KeepNick;

impl KeepNick {
    pub fn new() -> KeepNick {
        KeepNick {}
    }

    fn check_nick(&self, server: &IrcClient, leaver: &str) -> Result<(), IrcError> {
        let cfg_nick = match server.config().nickname {
            Some(ref nick) => nick.clone(),
            None => return Ok(()),
        };

        if leaver != cfg_nick {
            return Ok(());
        }

        let server_nick = server.current_nickname();

        if server_nick != cfg_nick {
            info!("Trying to switch nick from {} to {}", server_nick, cfg_nick);
            server.send(Command::NICK(cfg_nick))

        } else {
            Ok(())
        }
    }
}

impl Plugin for KeepNick {
    fn is_allowed(&self, _: &IrcClient, message: &Message) -> bool {
        match message.command {
            Command::QUIT(_) => true,
            _ => false,
        }
    }

    fn execute(&self, server: &IrcClient, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::QUIT(ref nick) => {
                self.check_nick(server, &nick.clone().unwrap_or_else(|| String::new()))
            }
            _ => Ok(()),
        }
    }

    fn command(&self, server: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("This Plugin does not implement any commands."))
    }
}

#[cfg(test)]
mod tests {}
