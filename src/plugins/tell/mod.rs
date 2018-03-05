use irc::client::prelude::*;

use std::time::Duration;
use std::sync::Mutex;

use time;
use chrono::NaiveDateTime;
use humantime::format_duration;

use plugin::*;

use failure::Fail;
use failure::ResultExt;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use self::error::*;

pub mod database;
use self::database::Database;

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

    fn tell_command(
        &self,
        client: &IrcClient,
        command: PluginCommand,
    ) -> Result<String, TellError> {
        if command.tokens.len() < 2 {
            return Ok(self.invalid_command(client));
        }

        let receiver = &command.tokens[0];
        let sender = command.source;

        if receiver.eq_ignore_ascii_case(&sender) {
            return Ok(String::from("That's your name!"));
        }

        if let Some(channels) = client.list_channels() {
            for channel in channels {
                if let Some(users) = client.list_users(&channel) {
                    if users
                        .iter()
                        .any(|u| u.get_nickname().eq_ignore_ascii_case(&receiver))
                    {
                        return Ok(format!("{} is currently online.", receiver));
                    }
                }
            }
        }

        let tm = time::now().to_timespec();
        let message = command.tokens[1..].join(" ");
        let tell = database::NewTellMessage {
            sender: &sender,
            receiver: &receiver.to_lowercase(),
            time: NaiveDateTime::from_timestamp(tm.sec, 0u32),
            message: &message,
        };

        try_lock!(self.tells).insert_tell(&tell)?;

        Ok(String::from("Got it!"))
    }

    fn send_tells(&self, client: &IrcClient, receiver: &str) -> ExecutionStatus {
        let mut tells = try_lock!(self.tells);

        let tell_messages = match tells.get_tells(&receiver.to_lowercase()) {
            Ok(t) => t,
            Err(e) => {
                // This warning only occurs if frippy is built without a database
                #[allow(unreachable_patterns)]
                return match e.kind() {
                    ErrorKind::NotFound => ExecutionStatus::Done,
                    _ => ExecutionStatus::Err(e.context(FrippyErrorKind::Tell).into()),
                };
            }
        };

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
                return ExecutionStatus::Err(e.context(FrippyErrorKind::Connection).into());
            }
            debug!(
                "Sent {:?} from {:?} to {:?}",
                tell.message, tell.sender, receiver
            );
        }

        if let Err(e) = tells.delete_tells(&receiver.to_lowercase()) {
            return ExecutionStatus::Err(e.context(FrippyErrorKind::Tell).into());
        };

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
            Command::JOIN(_, _, _) => self.send_tells(client, message.source_nickname().unwrap()),
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), FrippyError> {
        panic!("Tell should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), FrippyError> {
        if command.tokens.is_empty() {
            return Ok(client
                .send_notice(&command.source, &self.invalid_command(client))
                .context(FrippyErrorKind::Connection)?);
        }

        let sender = command.source.to_owned();

        Ok(match command.tokens[0].as_ref() {
            "help" => client
                .send_notice(&command.source, &self.help(client))
                .context(FrippyErrorKind::Connection)
                .into(),
            _ => match self.tell_command(client, command) {
                Ok(msg) => client
                    .send_notice(&sender, &msg)
                    .context(FrippyErrorKind::Connection),
                Err(e) => client
                    .send_notice(&sender, &e.to_string())
                    .context(FrippyErrorKind::Connection)
                    .into(),
            },
        }?)
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

pub mod error {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "TellError"]
    pub enum ErrorKind {
        /// Not found command error
        #[fail(display = "Tell was not found")]
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
