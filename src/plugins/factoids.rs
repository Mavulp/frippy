
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use std::collections::HashMap;
use std::sync::Mutex;

use plugin::*;

#[derive(PluginName, Debug)]
pub struct Factoids {
    factoids: Mutex<HashMap<String, String>>,
}

macro_rules! try_lock {
    ( $m:expr ) => {
        match $m.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Factoids {
    pub fn new() -> Factoids {
        Factoids { factoids: Mutex::new(HashMap::new()) }
    }

    fn add(&self, server: &IrcServer, command: &mut PluginCommand) -> Result<(), IrcError> {

        if command.tokens.len() < 2 {
            return self.invalid_command(server, command);
        }

        let name = command.tokens.remove(0);

        try_lock!(self.factoids)
            .insert(name, command.tokens.join(" "));

        server.send_notice(&command.source, "Successfully added")
    }

    fn get(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        if command.tokens.len() < 1 {
            self.invalid_command(server, command)

        } else {
            let factoids = try_lock!(self.factoids);
            let factoid = match factoids.get(&command.tokens[0]) {
                Some(v) => v,
                None => return self.invalid_command(server, command),
            };

            server.send_privmsg(&command.target, factoid)
        }
    }

    fn invalid_command(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source, "Invalid Command")
    }
}

impl Plugin for Factoids {
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
            match message.command {
                Command::PRIVMSG(_, ref content) => content.starts_with('!'),
                _ => false,
            }
    }

    fn execute(&self, server: &IrcServer, message: &Message) -> Result<(), IrcError> {
        if let Command::PRIVMSG(_, mut content) = message.command.clone() {
            content.remove(0);

            let t: Vec<String> = content
                .split(' ')
                .map(ToOwned::to_owned)
                .collect();

            let c = PluginCommand {
                source: message.source_nickname().unwrap().to_string(),
                target: message.response_target().unwrap().to_string(),
                tokens: t,
            };

            self.get(server, &c)

        } else {
            Ok(())
        }
    }

    fn command(&self, server: &IrcServer, mut command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            self.invalid_command(server, &command)

        } else if command.tokens[0].to_lowercase() == "add" {
            command.tokens.remove(0);
            self.add(server, &mut command)

        } else if command.tokens[0].to_lowercase() == "get" {
            command.tokens.remove(0);
            self.get(server, &command)

        } else {
            self.invalid_command(server, &command)
        }
    }
}
