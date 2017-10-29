extern crate rlua;

use self::rlua::prelude::*;
use irc::client::prelude::*;
use irc::error::Error as IrcError;

use std::sync::Mutex;

use plugin::*;
mod database;
use self::database::*;

static LUA_SANDBOX: &'static str = include_str!("sandbox.lua");

#[derive(PluginName, Debug)]
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
        Factoids { factoids: Mutex::new(db) }
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
            let name = &command.tokens[0];
            let factoids = try_lock!(self.factoids);
            let factoid = match factoids.get(name) {
                Some(v) => v,
                None => return self.invalid_command(server, command),
            };

            server.send_privmsg(&command.target, &format!("{}: {}", name, factoid))
        }
    }

    fn exec(&self, server: &IrcServer, mut command: PluginCommand) -> Result<(), IrcError> {

        if command.tokens.len() < 1 {
            self.invalid_command(server, &command)

        } else {
            let name = command.tokens.remove(0);

            let factoids = try_lock!(self.factoids);
            let factoid = match factoids.get(&name) {
                Some(v) => v,
                None => return self.invalid_command(server, &command),
            };

            let value = if factoid.starts_with(">") {
                let factoid = String::from(&factoid[1..]);

                if factoid.starts_with(">") {
                    factoid
                } else {
                    match self.run_lua(&name, &factoid, &command) {
                        Ok(v) => v,
                        Err(e) => format!("{}", e),
                    }
                }
            } else {
                String::from(factoid)
            };

            server.send_privmsg(&command.target, &value)
        }
    }

    fn run_lua(&self,
               name: &str,
               code: &str,
               command: &PluginCommand)
               -> Result<String, rlua::Error> {

        let args = command
            .tokens
            .iter()
            .filter(|x| !x.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<String>>();

        let lua = Lua::new();
        let globals = lua.globals();

        globals.set("factoid", lua.load(code, Some(name))?)?;
        globals.set("args", args)?;
        globals.set("input", command.tokens.join(" "))?;
        globals.set("user", command.source.clone())?;
        globals.set("channel", command.target.clone())?;
        globals.set("output", lua.create_table())?;

        lua.exec::<()>(LUA_SANDBOX, Some(name))?;
        let output: Vec<String> = globals.get::<_, Vec<String>>("output")?;

        Ok(output.join("|").replace("\n", "|"))
    }

    fn invalid_command(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source, "Invalid Command")
    }
}

impl<T: Database> Plugin for Factoids<T> {
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
        match message.command {
            Command::PRIVMSG(_, ref content) => content.starts_with('!'),
            _ => false,
        }
    }

    fn execute(&self, server: &IrcServer, message: &Message) -> Result<(), IrcError> {
        if let Command::PRIVMSG(_, mut content) = message.command.clone() {
            content.remove(0);

            let t: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            let c = PluginCommand {
                source: message.source_nickname().unwrap().to_string(),
                target: message.response_target().unwrap().to_string(),
                tokens: t,
            };

            self.exec(server, c)

        } else {
            Ok(())
        }
    }

    fn command(&self, server: &IrcServer, mut command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            return self.invalid_command(server, &command);
        }

        let sub_command = command.tokens.remove(0);
        match sub_command.as_ref() {
            "add" => self.add(server, &mut command),
            "get" => self.get(server, &command),
            "exec" => self.exec(server, command),
            _ => self.invalid_command(server, &command),
        }
    }
}
