use irc::client::prelude::*;
use irc::error::IrcError;

use std::time::Duration;
use std::sync::Mutex;

use time;
use chrono::NaiveDateTime;
use humantime::format_duration;

use plugin::*;

pub mod database;
use self::database::{Database, DbResponse};

macro_rules! try_lock {
    ( $m:expr ) => {
        match $m.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(PluginName, Default)]
pub struct Tell<T: Database> {
    tells: Mutex<T>,
}

impl<T: Database> Tell<T> {
    pub fn new(db: T) -> Tell<T> {
        Tell {
            tells: Mutex::new(db),
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

        let tm = time::now().to_timespec();
        let message = command.tokens[1..].join(" ");
        let tell = database::NewTellMessage {
            sender: &sender,
            receiver: &receiver,
            time: NaiveDateTime::from_timestamp(tm.sec, 0u32),
            message: &message,
        };

        match try_lock!(self.tells).insert_tell(&tell) {
            DbResponse::Success => Ok("Got it!"),
            DbResponse::Failed(e) => Err(e.to_string()),
        }
    }

    fn send_tell(&self, client: &IrcClient, receiver: &str) -> ExecutionStatus {
        let mut tells = try_lock!(self.tells);
        if let Some(tell_messages) = tells.get_tells(receiver) {
            for tell in tell_messages {
                let now = Duration::new(time::now().to_timespec().sec as u64, 0);
                let dur = now - Duration::new(tell.time.timestamp() as u64, 0);
                let human_dur = format_duration(dur);

                if let Err(e) = client.send_notice(
                    receiver,
                    &format!(
                        "Tell from {} {} ago: {}",
                        tell.sender, human_dur, tell.message
                    ),
                ) {
                    return ExecutionStatus::Err(Box::new(e));
                }
                debug!(
                    "Sent {:?} from {:?} to {:?}",
                    tell.message, tell.sender, receiver
                );
            }
        }
        tells.delete_tells(receiver);
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

impl<T: Database> Plugin for Tell<T> {
    fn execute(&self, client: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::JOIN(_, _, _) | Command::PRIVMSG(_, _) => {
                self.send_tell(client, message.source_nickname().unwrap())
            }
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

use std::fmt;
impl<T: Database> fmt::Debug for Tell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tell {{ ... }}")
    }
}
