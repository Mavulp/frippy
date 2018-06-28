extern crate rlua;

use self::rlua::prelude::*;
use antidote::RwLock;
use irc::client::prelude::*;
use std::fmt;
use std::str::FromStr;

use chrono::NaiveDateTime;
use time;

use plugin::*;
pub mod database;
use self::database::Database;

mod utils;
use self::utils::*;
use utils::Url;

use self::error::*;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::ResultExt;

static LUA_SANDBOX: &'static str = include_str!("sandbox.lua");

enum FactoidResponse {
    Public(String),
    Private(String),
}

#[derive(PluginName)]
pub struct Factoids<T: Database> {
    factoids: RwLock<T>,
}

impl<T: Database> Factoids<T> {
    pub fn new(db: T) -> Factoids<T> {
        Factoids {
            factoids: RwLock::new(db),
        }
    }

    fn create_factoid(
        &self,
        name: &str,
        content: &str,
        author: &str,
    ) -> Result<&str, FactoidsError> {
        let count = self.factoids.read().count_factoids(name)?;
        let tm = time::now().to_timespec();

        let factoid = database::NewFactoid {
            name,
            idx: count,
            content,
            author,
            created: NaiveDateTime::from_timestamp(tm.sec, 0u32),
        };

        Ok(self.factoids
            .write()
            .insert_factoid(&factoid)
            .map(|()| "Successfully added!")?)
    }

    fn add(&self, command: &mut PluginCommand) -> Result<&str, FactoidsError> {
        if command.tokens.len() < 2 {
            Err(ErrorKind::InvalidCommand)?;
        }

        let name = command.tokens.remove(0);
        let content = command.tokens.join(" ");

        Ok(self.create_factoid(&name, &content, &command.source)?)
    }

    fn add_from_url(&self, command: &mut PluginCommand) -> Result<&str, FactoidsError> {
        if command.tokens.len() < 2 {
            Err(ErrorKind::InvalidCommand)?;
        }

        let name = command.tokens.remove(0);
        let url = &command.tokens[0];
        let content = Url::from(url.as_ref())
            .max_kib(1024)
            .request()
            .context(ErrorKind::Download)?;

        Ok(self.create_factoid(&name, &content, &command.source)?)
    }

    fn remove(&self, command: &mut PluginCommand) -> Result<&str, FactoidsError> {
        if command.tokens.is_empty() {
            Err(ErrorKind::InvalidCommand)?;
        }

        let name = command.tokens.remove(0);
        let count = self.factoids.read().count_factoids(&name)?;

        match self.factoids.write().delete_factoid(&name, count - 1) {
            Ok(()) => Ok("Successfully removed"),
            Err(e) => Err(e)?,
        }
    }

    fn get(&self, command: &PluginCommand) -> Result<String, FactoidsError> {
        let (name, idx) = match command.tokens.len() {
            0 => Err(ErrorKind::InvalidCommand)?,
            1 => {
                let name = &command.tokens[0];
                let count = self.factoids.read().count_factoids(name)?;

                if count < 1 {
                    Err(ErrorKind::NotFound)?;
                }

                (name, count - 1)
            }
            _ => {
                let name = &command.tokens[0];
                let idx = match i32::from_str(&command.tokens[1]) {
                    Ok(i) => i,
                    Err(_) => Err(ErrorKind::InvalidCommand)?,
                };

                (name, idx)
            }
        };

        let factoid = self.factoids
            .read()
            .get_factoid(name, idx)
            .context(ErrorKind::NotFound)?;

        let message = factoid.content.replace("\n", "|").replace("\r", "");

        Ok(format!("{}: {}", factoid.name, message))
    }

    fn info(&self, command: &PluginCommand) -> Result<String, FactoidsError> {
        match command.tokens.len() {
            0 => Err(ErrorKind::InvalidCommand)?,
            1 => {
                let name = &command.tokens[0];
                let count = self.factoids.read().count_factoids(name)?;

                Ok(match count {
                    0 => Err(ErrorKind::NotFound)?,
                    1 => format!("There is 1 version of {}", name),
                    _ => format!("There are {} versions of {}", count, name),
                })
            }
            _ => {
                let name = &command.tokens[0];
                let idx = i32::from_str(&command.tokens[1]).context(ErrorKind::InvalidIndex)?;
                let factoid = self.factoids.read().get_factoid(name, idx)?;

                Ok(format!(
                    "{}: Added by {} at {} UTC",
                    name, factoid.author, factoid.created
                ))
            }
        }
    }

    fn exec(&self, mut command: PluginCommand) -> Result<String, FactoidsError> {
        if command.tokens.is_empty() {
            Err(ErrorKind::InvalidIndex)?
        } else {
            let name = command.tokens.remove(0);
            let count = self.factoids.read().count_factoids(&name)?;
            let factoid = self.factoids.read().get_factoid(&name, count - 1)?;

            let content = factoid.content;
            let value = if content.starts_with('>') {
                let content = String::from(&content[1..]);

                if content.starts_with('>') {
                    content
                } else {
                    match self.run_lua(&name, &content, &command) {
                        Ok(v) => v,
                        Err(e) => match e {
                            LuaError::CallbackError { cause, .. } => cause.to_string(),
                            _ => e.to_string(),
                        },
                    }
                }
            } else {
                content
            };

            Ok(value.replace("\n", "|").replace("\r", ""))
        }
    }

    fn run_lua(&self, name: &str, code: &str, command: &PluginCommand) -> Result<String, LuaError> {
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
        globals.set("json_decode", lua.create_function(json_decode)?)?;
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

    fn help(&self) -> &str {
        "usage: factoids <subcommand>\r\n\
         subcommands: add, fromurl, remove, info, get, exec, help"
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

    fn execute_threaded(&self, client: &IrcClient, message: &Message) -> Result<(), FrippyError> {
        if let Command::PRIVMSG(_, mut content) = message.command.clone() {
            content.remove(0);

            let t: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            let c = PluginCommand {
                source: message.source_nickname().unwrap().to_owned(),
                target: message.response_target().unwrap().to_owned(),
                tokens: t,
            };

            if let Ok(f) = self.exec(c) {
                client
                    .send_privmsg(&message.response_target().unwrap(), &f)
                    .context(FrippyErrorKind::Connection)?;
            }
        }

        Ok(())
    }

    fn command(&self, client: &IrcClient, mut command: PluginCommand) -> Result<(), FrippyError> {
        use self::FactoidResponse::{Private, Public};

        if command.tokens.is_empty() {
            client
                .send_notice(&command.source, "Invalid command")
                .context(FrippyErrorKind::Connection)?;

            return Ok(());
        }

        let target = command.target.clone();
        let source = command.source.clone();

        let sub_command = command.tokens.remove(0);
        let result = match sub_command.as_ref() {
            "add" => self.add(&mut command).map(|s| Private(s.to_owned())),
            "fromurl" => self.add_from_url(&mut command)
                .map(|s| Private(s.to_owned())),
            "remove" => self.remove(&mut command).map(|s| Private(s.to_owned())),
            "get" => self.get(&command).map(Public),
            "info" => self.info(&command).map(Public),
            "exec" => self.exec(command).map(Public),
            "help" => Ok(Private(self.help().to_owned())),
            _ => Err(ErrorKind::InvalidCommand.into()),
        };

        match result {
            Ok(v) => match v {
                Public(m) => client
                    .send_privmsg(&target, &m)
                    .context(FrippyErrorKind::Connection)?,
                Private(m) => client
                    .send_notice(&source, &m)
                    .context(FrippyErrorKind::Connection)?,
            },
            Err(e) => {
                let message = e.to_string();
                client
                    .send_notice(&source, &message)
                    .context(FrippyErrorKind::Connection)?;
                Err(e).context(FrippyErrorKind::Factoids)?
            }
        }

        Ok(())
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

pub mod error {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "FactoidsError"]
    pub enum ErrorKind {
        /// Invalid command error
        #[fail(display = "Invalid Command")]
        InvalidCommand,

        /// Invalid index error
        #[fail(display = "Invalid index")]
        InvalidIndex,

        /// Download  error
        #[fail(display = "Download failed")]
        Download,

        /// Duplicate error
        #[fail(display = "Entry already exists")]
        Duplicate,

        /// Not found error
        #[fail(display = "Factoid was not found")]
        NotFound,

        /// MySQL error
        #[cfg(feature = "mysql")]
        #[fail(display = "Failed to execute MySQL Query")]
        MysqlError,

        /// No connection error
        #[cfg(feature = "mysql")]
        #[fail(display = "No connection to the database")]
        NoConnection,
    }
}
