extern crate rlua;

use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use self::rlua::prelude::*;
use irc::client::prelude::*;
use irc::error::IrcError;
use error::FrippyError;
use error::PluginError;
use failure::Fail;

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

    fn create_factoid(&self, name: &str, content: &str, author: &str) -> Result<&str, FrippyError> {
        let count = try_lock!(self.factoids).count_factoids(name).map_err(|e| PluginError::Factoids { error: e.to_owned() })?;
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
            DbResponse::Failed(e) => Err(PluginError::Factoids { error: e.to_owned() })?,
        }
    }

    fn add(&self, client: &IrcClient, command: &mut PluginCommand) -> Result<&str, FrippyError> {
        if command.tokens.len() < 2 {
            return Ok(self.invalid_command(client, command).map(|()| "")?);
        }

        let name = command.tokens.remove(0);
        let content = command.tokens.join(" ");

        Ok(self.create_factoid(&name, &content, &command.source)?)
    }

    fn add_from_url(
        &self,
        client: &IrcClient,
        command: &mut PluginCommand,
    ) -> Result<&str, FrippyError> {
        if command.tokens.len() < 2 {
            return Ok(self.invalid_command(client, command).map(|()| "")?);
        }

        let name = command.tokens.remove(0);
        let url = &command.tokens[0];
        let content = ::utils::download(url, Some(1024))?;

        Ok(self.create_factoid(&name, &content, &command.source)?)
    }

    fn remove(&self, client: &IrcClient, command: &mut PluginCommand) -> Result<&str, FrippyError> {
        if command.tokens.len() < 1 {
            return Ok(self.invalid_command(client, command).map(|()| "")?);
        }

        let name = command.tokens.remove(0);
        let count = try_lock!(self.factoids).count_factoids(&name).map_err(|e| PluginError::Factoids { error: e.to_owned() } )?;

        match try_lock!(self.factoids).delete_factoid(&name, count - 1) {
            DbResponse::Success => Ok("Successfully removed"),
            DbResponse::Failed(e) => Err(PluginError::Factoids { error: e.to_owned() })?,
        }
    }

    fn get(&self, client: &IrcClient, command: &PluginCommand) -> Result<String, FrippyError> {
        let (name, idx) = match command.tokens.len() {
            0 => return Ok(self.invalid_command(client, command).map(|()| String::new())?),
            1 => {
                let name = &command.tokens[0];
                let count = try_lock!(self.factoids).count_factoids(name).map_err(|e| PluginError::Factoids { error: e.to_owned() } )?;

                if count < 1 {
                    Err(PluginError::Factoids { error: format!("{} does not exist", name) })?;
                }

                (name, count - 1)
            }
            _ => {
                let name = &command.tokens[0];
                let idx = match i32::from_str(&command.tokens[1]) {
                    Ok(i) => i,
                    Err(_) => Err(PluginError::Factoids { error: String::from("Invalid index") })?,
                };

                (name, idx)
            }
        };

        let factoid = match try_lock!(self.factoids).get_factoid(name, idx) {
            Some(v) => v,
            None => Err(PluginError::Factoids { error: format!("{}~{} does not exist", name, idx) })?,
        };

        let message = factoid.content.replace("\n", "|").replace("\r", "");

        Ok(format!("{}: {}", factoid.name, message))
    }

    fn info(&self, client: &IrcClient, command: &PluginCommand) -> Result<String, FrippyError> {
        match command.tokens.len() {
            0 => Ok(self.invalid_command(client, command).map(|()| String::new())?),
            1 => {
                let name = &command.tokens[0];
                let count = try_lock!(self.factoids).count_factoids(name).map_err(|e| PluginError::Factoids { error: e.to_owned() } )?;

                Ok(match count {
                    0 => Err(PluginError::Factoids { error: format!("{} does not exist", name) })?,
                    1 => format!("There is 1 version of {}", name),
                    _ => format!("There are {} versions of {}", count, name),
                })
            }
            _ => {
                let name = &command.tokens[0];
                let idx = i32::from_str(&command.tokens[1]).map_err(|_| PluginError::Factoids { error: String::from("Invalid index") })?;

                let factoid = match try_lock!(self.factoids).get_factoid(name, idx) {
                    Some(v) => v,
                    None => return Ok(format!("{}~{} does not exist", name, idx)),
                };

                Ok(format!("{}: Added by {} at {} UTC", name, factoid.author, factoid.created))
            }
        }
    }

    fn exec(
        &self,
        client: &IrcClient,
        mut command: PluginCommand,
    ) -> Result<String, FrippyError> {
        if command.tokens.len() < 1 {
            Ok(self.invalid_command(client, &command).map(|()| String::new())?)
        } else {
            let name = command.tokens.remove(0);
            let count = try_lock!(self.factoids).count_factoids(&name).map_err(|e| PluginError::Factoids { error: e.to_owned() } )?;
            let factoid = try_lock!(self.factoids).get_factoid(&name, count - 1).ok_or(PluginError::Factoids { error: format!("The factoid \"{}\" does not exist", name) })?;

            let content = factoid.content;
            let value = if content.starts_with('>') {
                let content = String::from(&content[1..]);

                if content.starts_with('>') {
                    content
                } else {
                    match self.run_lua(&name, &content, &command) {
                        Ok(v) => v,
                        Err(e) => format!("\"{}\"", e),
                    }
                }
            } else {
                content
            };

            Ok(value.replace("\n", "|").replace("\r", ""))
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
                source: message.source_nickname().unwrap().to_owned(),
                target: message.response_target().unwrap().to_owned(),
                tokens: t,
            };

            match self.exec(client, c) {
                Ok(f) => client.send_privmsg(&message.response_target().unwrap(), &f),
                Err(_) => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    fn command(&self, client: &IrcClient, mut command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            return self.invalid_command(client, &command);
        }

        let target = command.target.clone();
        let source = command.source.clone();

        let sub_command = command.tokens.remove(0);
        let result = match sub_command.as_ref() {
            "add" => self.add(client, &mut command).map(|s| s.to_owned()),
            "fromurl" => self.add_from_url(client, &mut command).map(|s| s.to_owned()),
            "remove" => self.remove(client, &mut command).map(|s| s.to_owned()),
            "get" => self.get(client, &command),
            "info" => self.info(client, &command),
            "exec" => self.exec(client, command),
            _ => self.invalid_command(client, &command).map(|()| String::new()).map_err(|e| e.into()),
        };

        Ok(match result {
            Ok(v) => client.send_privmsg(&target, &v),
            Err(e) => client.send_notice(&source, &e.cause().unwrap().to_string()),
        }?)
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
