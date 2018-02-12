use irc::client::prelude::*;
use irc::error::IrcError;

use plugin::*;

#[derive(PluginName, Default, Debug)]
pub struct KeepNick;

impl KeepNick {
    pub fn new() -> KeepNick {
        KeepNick {}
    }

    fn check_nick(&self, client: &IrcClient, leaver: &str) -> ExecutionStatus {
        let cfg_nick = match client.config().nickname {
            Some(ref nick) => nick.clone(),
            None => return ExecutionStatus::Done,
        };

        if leaver != cfg_nick {
            return ExecutionStatus::Done;
        }

        let client_nick = client.current_nickname();

        if client_nick != cfg_nick {
            info!("Trying to switch nick from {} to {}", client_nick, cfg_nick);
            match client.send(Command::NICK(cfg_nick)) {
                Ok(_) => ExecutionStatus::Done,
                Err(e) => ExecutionStatus::Err(e),
            }

        } else {
            ExecutionStatus::Done
        }
    }
}

impl Plugin for KeepNick {
    fn execute(&self, client: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::QUIT(ref nick) => {
                self.check_nick(client, &nick.clone().unwrap_or_else(String::new))
            }
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), IrcError> {
        panic!("Tell should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        client.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("This Plugin does not implement any commands."))
    }
}

#[cfg(test)]
mod tests {}
