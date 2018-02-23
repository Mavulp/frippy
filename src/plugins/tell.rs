use irc::client::prelude::*;
use irc::error::IrcError;

use std::collections::HashMap;
use std::sync::Mutex;

use plugin::*;

macro_rules! try_lock {
    ( $m:expr ) => {
        match $m.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(PluginName, Default, Debug)]
pub struct Tell {
    tells: Mutex<HashMap<String, Vec<TellMessage>>>,
}

#[derive(Default, Debug)]
struct TellMessage {
    sender: String,
    // TODO Add time
    message: String,
}

impl Tell {
    pub fn new() -> Tell {
        Tell {
            tells: Mutex::new(HashMap::new()),
        }
    }

    fn tell_command(&self, client: &IrcClient, command: &PluginCommand) -> Result<&str, String> {
        if command.tokens.len() < 2 {
            return Err(self.invalid_command(client));
        }

        let receiver = command.tokens[0].to_string();
        let sender = command.source.to_owned();

        if receiver == sender {
            return Err(String::from("That's your name!"));
        }

        if command.source != command.target {
            if let Some(users) = client.list_users(&command.target) {
                if users.iter().any(|u| u.get_nickname() == receiver) {
                    return Err(format!("{} is in this channel.", receiver));
                }
            }
        }

        let message = command.tokens[1..].join(" ");
        let tell = TellMessage {
            sender: sender,
            message: message,
        };

        let mut tells = try_lock!(self.tells);
        let tell_messages = tells.entry(receiver).or_insert(Vec::with_capacity(3));
        (*tell_messages).push(tell);

        Ok("Got it!")
    }

    fn send_tell(&self, client: &IrcClient, receiver: &str) -> ExecutionStatus {
        let mut tells = try_lock!(self.tells);
        if let Some(tell_messages) = tells.get_mut(receiver) {
            for tell in tell_messages {
                if let Err(e) = client.send_notice(
                    receiver,
                    &format!("Tell from {}: {}", tell.sender, tell.message),
                ) {
                    return ExecutionStatus::Err(e);
                }
                debug!(
                    "Sent {:?} from {:?} to {:?}",
                    tell.message, tell.sender, receiver
                );
            }
        }
        tells.remove(receiver);
        ExecutionStatus::Done
    }

    fn invalid_command(&self, client: &IrcClient) -> String {
        format!(
            "Incorrect Command. \
             Send \"{} tell help\" for help.",
            client.current_nickname()
        )
    }

    fn help(&self, client: &IrcClient) -> String {
        format!(
            "usage: {} tell user message\r\n\
             example: {0} tell Foobar Hello!",
            client.current_nickname()
        )
    }
}

impl Plugin for Tell {
    fn execute(&self, client: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::JOIN(_, _, _) => self.send_tell(client, message.source_nickname().unwrap()),
            Command::PRIVMSG(_, _) => self.send_tell(client, message.source_nickname().unwrap()),
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), IrcError> {
        panic!("Tell should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            return client.send_notice(&command.source, &self.invalid_command(client));
        }

        match command.tokens[0].as_ref() {
            "help" => client.send_notice(&command.source, &self.help(client)),
            _ => match self.tell_command(client, &command) {
                Ok(msg) => client.send_notice(&command.source, msg),
                Err(msg) => client.send_notice(&command.source, &msg),
            },
        }
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("This Plugin does not implement any commands."))
    }
}

#[cfg(test)]
mod tests {}
