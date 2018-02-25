extern crate rlua;

use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use self::rlua::prelude::*;
use irc::client::prelude::*;
use irc::error::IrcError;

use time;
use chrono::NaiveDateTime;

use plugin::*;
pub mod database;
use self::database::{Database, DbResponse};

mod utils;
use self::utils::*;

static LUA_SANDBOX: &'static str = include_str!("sandbox.lua");

#[derive(PluginName)]
pub struct Factoids<T: Database> {
    factoids: Mutex<T>,
}

macro_rules! try_lock {
    ( $m:expr ) => {
        match $m.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl<T: Database> Factoids<T> {
    pub fn new(db: T) -> Factoids<T> {
        Factoids {
            factoids: Mutex::new(db),
        }
    }

    fn create_factoid(&self, name: &str, content: &str, author: &str) -> Result<&str, &str> {
        let count = try_lock!(self.factoids).count_factoids(name)?;
        let tm = time::now().to_timespec();

        let factoid = database::NewFactoid {
            name: name,
            idx: count,
            content: content,
            author: author,
            created: NaiveDateTime::from_timestamp(tm.sec, 0u32),
        };

        match try_lock!(self.factoids).insert_factoid(&factoid) {
            DbResponse::Success => Ok("Successfully added"),
            DbResponse::Failed(e) => Err(e),
        }
    }

    fn add(&self, client: &IrcClient, command: &mut PluginCommand) -> Result<(), IrcError> {
        if command.tokens.len() < 2 {
            return self.invalid_command(client, command);
        }

        let name = command.tokens.remove(0);
        let content = command.tokens.join(" ");

        match self.create_factoid(&name, &content, &command.source) {
            Ok(v) => client.send_notice(&command.source, v),
            Err(e) => client.send_notice(&command.source, e),
        }
    }

    fn save_from_url(
        &self,
        client: &IrcClient,
        command: &mut PluginCommand,
    ) -> Result<(), IrcError> {
        if command.tokens.len() < 2 {
            return self.invalid_command(client, command);
        }

        let name = command.tokens.remove(0);
        let url = &command.tokens[0];
        if let Some(content) = ::utils::download(1024, url) {
            match self.create_factoid(&name, &content, &command.source) {
                Ok(v) => client.send_notice(&command.source, v),
                Err(e) => client.send_notice(&command.source, e),
            }
        } else {
            client.send_notice(&command.source, "Failed to download.")
        }
    }

    fn remove(&self, client: &IrcClient, command: &mut PluginCommand) -> Result<(), IrcError> {
        if command.tokens.len() < 1 {
            return self.invalid_command(client, command);
        }

        let name = command.tokens.remove(0);
        let count = match try_lock!(self.factoids).count_factoids(&name) {
            Ok(c) => c,
            Err(e) => return client.send_notice(&command.source, e),
        };

        match try_lock!(self.factoids).delete_factoid(&name, count - 1) {
            DbResponse::Success => client.send_notice(&command.source, "Successfully removed"),
            DbResponse::Failed(e) => client.send_notice(&command.source, e),
        }
    }

    fn get(&self, client: &IrcClient, command: &PluginCommand) -> Result<(), IrcError> {
        let (name, idx) = match command.tokens.len() {
            0 => return self.invalid_command(client, command),
            1 => {
                let name = &command.tokens[0];
                let count = match try_lock!(self.factoids).count_factoids(name) {
                    Ok(c) => c,
                    Err(e) => return client.send_notice(&command.source, e),
                };

                if count < 1 {
                    return client.send_notice(&command.source, &format!("{} does not exist", name));
                }

                (name, count - 1)
            }
            _ => {
                let name = &command.tokens[0];
                let idx = match i32::from_str(&command.tokens[1]) {
                    Ok(i) => i,
                    Err(_) => return client.send_notice(&command.source, "Invalid index"),
                };

                (name, idx)
            }
        };

        let factoid = match try_lock!(self.factoids).get_factoid(name, idx) {
            Some(v) => v,
            None => {
                return client
                    .send_notice(&command.source, &format!("{}~{} does not exist", name, idx))
            }
        };

        let message = factoid.content.replace("\n", "|").replace("\r", "");

        client.send_privmsg(&command.target, &format!("{}: {}", factoid.name, message))
    }

    fn info(&self, client: &IrcClient, command: &PluginCommand) -> Result<(), IrcError> {
        match command.tokens.len() {
            0 => self.invalid_command(client, command),
            1 => {
                let name = &command.tokens[0];
                let count = match try_lock!(self.factoids).count_factoids(name) {
                    Ok(c) => c,
                    Err(e) => return client.send_notice(&command.source, e),
                };

                match count {
                    0 => client.send_notice(&command.source, &format!("{} does not exist", name)),
                    1 => client
                        .send_privmsg(&command.target, &format!("There is 1 version of {}", name)),
                    _ => client.send_privmsg(
                        &command.target,
                        &format!("There are {} versions of {}", count, name),
                    ),
                }
            }
            _ => {
                let name = &command.tokens[0];
                let idx = match i32::from_str(&command.tokens[1]) {
                    Ok(i) => i,
                    Err(_) => return client.send_notice(&command.source, "Invalid index"),
                };

                let factoid = match try_lock!(self.factoids).get_factoid(name, idx) {
                    Some(v) => v,
                    None => {
                        return client.send_notice(
                            &command.source,
                            &format!("{}~{} does not exist", name, idx),
                        )
                    }
                };

                client.send_privmsg(
                    &command.target,
                    &format!(
                        "{}: Added by {} at {} UTC",
                        name, factoid.author, factoid.created
                    ),
                )
            }
        }
    }

    fn exec(
        &self,
        client: &IrcClient,
        mut command: PluginCommand,
        error: bool,
    ) -> Result<(), IrcError> {
        if command.tokens.len() < 1 {
            self.invalid_command(client, &command)
        } else {
            let name = command.tokens.remove(0);
            let count = match try_lock!(self.factoids).count_factoids(&name) {
                Ok(c) => c,
                Err(e) => return client.send_notice(&command.source, e),
            };

            let factoid = match try_lock!(self.factoids).get_factoid(&name, count - 1) {
                Some(v) => v.content,
                None if error => return self.invalid_command(client, &command),
                None => return Ok(()),
            };

            let value = &if factoid.starts_with('>') {
                let factoid = String::from(&factoid[1..]);

                if factoid.starts_with('>') {
                    factoid
                } else {
                    match self.run_lua(&name, &factoid, &command) {
                        Ok(v) => v,
                        Err(e) => format!("\"{}\"", e),
                    }
                }
            } else {
                factoid
            };

            client.send_privmsg(&command.target, &value.replace("\n", "|").replace("\r", ""))
        }
    }

    fn run_lua(
        &self,
        name: &str,
        code: &str,
        command: &PluginCommand,
    ) -> Result<String, rlua::Error> {
        let args = command
            .tokens
            .iter()
            .filter(|x| !x.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<String>>();

        let lua = unsafe { Lua::new_with_debug() };
        let globals = lua.globals();

        globals.set("factoid", code)?;
        globals.set("download", lua.create_function(download)?)?;
        globals.set("sleep", lua.create_function(sleep)?)?;
        globals.set("args", args)?;
        globals.set("input", command.tokens.join(" "))?;
        globals.set("user", command.source.clone())?;
        globals.set("channel", command.target.clone())?;
        globals.set("output", lua.create_table()?)?;

        lua.exec::<()>(LUA_SANDBOX, Some(name))?;
        let output: Vec<String> = globals.get::<_, Vec<String>>("output")?;

        Ok(output.join("|"))
    }

    fn invalid_command(&self, client: &IrcClient, command: &PluginCommand) -> Result<(), IrcError> {
        client.send_notice(&command.source, "Invalid Command")
    }
}

impl<T: Database> Plugin for Factoids<T> {
    fn execute(&self, _: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::PRIVMSG(_, ref content) => if content.starts_with('!') {
                ExecutionStatus::RequiresThread
            } else {
                ExecutionStatus::Done
            },
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, client: &IrcClient, message: &Message) -> Result<(), IrcError> {
        if let Command::PRIVMSG(_, mut content) = message.command.clone() {
            content.remove(0);

            let t: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            let c = PluginCommand {
                source: message.source_nickname().unwrap().to_string(),
                target: message.response_target().unwrap().to_string(),
                tokens: t,
            };

            self.exec(client, c, false)
        } else {
            Ok(())
        }
    }

    fn command(&self, client: &IrcClient, mut command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            return self.invalid_command(client, &command);
        }

        let sub_command = command.tokens.remove(0);
        match sub_command.as_ref() {
            "add" => self.add(client, &mut command),
            "fromurl" => self.save_from_url(client, &mut command),
            "remove" => self.remove(client, &mut command),
            "get" => self.get(client, &command),
            "info" => self.info(client, &command),
            "exec" => self.exec(client, command, true),
            _ => self.invalid_command(client, &command),
        }
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from(
            "Evaluation of commands is not implemented for Factoids at this time",
        ))
    }
}

impl<T: Database> fmt::Debug for Factoids<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Factoids {{ ... }}")
    }
}
