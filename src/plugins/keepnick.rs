use irc::client::prelude::*;
use irc::error::Error as IrcError;

use plugin::*;

#[derive(PluginName, Debug)]
pub struct KeepNick;

impl KeepNick {
    pub fn new() -> KeepNick {
        KeepNick {}
    }

    fn check_nick(&self, server: &IrcServer) -> Result<(), IrcError> {
        let cfg_nick = match server.config().nickname {
            Some(ref nick) => nick.clone(),
            None => return Ok(()),
        };

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
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
        match message.command {
            Command::QUIT(_) => true,
            _ => false,
        }
    }

    fn execute(&self, server: &IrcServer, _: &Message) -> Result<(), IrcError> {
        self.check_nick(server)
    }

    fn command(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }
}

#[cfg(test)]
mod tests {}
