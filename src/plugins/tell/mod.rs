use std::marker::PhantomData;

use antidote::RwLock;
use irc::client::data::User;
use irc::client::prelude::*;

use chrono::NaiveDateTime;
use humantime::format_duration;
use std::time::Duration;
use time;

use plugin::*;
use FrippyClient;

use self::error::*;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::Fail;
use failure::ResultExt;
use log::{debug, trace};

pub mod database;
use self::database::Database;

#[derive(PluginName)]
pub struct Tell<T: Database, C> {
    tells: RwLock<T>,
    phantom: PhantomData<C>,
}

impl<T: Database, C: FrippyClient> Tell<T, C> {
    pub fn new(db: T) -> Self {
        Tell {
            tells: RwLock::new(db),
            phantom: PhantomData,
        }
    }

    fn tell_command(&self, client: &C, command: PluginCommand) -> Result<String, TellError> {
        if command.tokens.len() < 2 {
            return Ok(self.invalid_command().to_owned());
        }

        let mut online = Vec::new();

        let receivers = command.tokens[0].split(',').filter(|&s| !s.is_empty());
        let sender = command.source;

        let mut no_receiver = true;
        for receiver in receivers {
            if receiver.eq_ignore_ascii_case(client.current_nickname())
                || receiver.eq_ignore_ascii_case(&sender)
            {
                if !online.contains(&receiver) {
                    online.push(receiver);
                }
                continue;
            }

            let channels = client
                .list_channels()
                .expect("The irc crate should not be compiled with the \"nochanlists\" feature");

            let find_receiver = |option: Option<Vec<User>>| {
                option.and_then(|users| {
                    users
                        .into_iter()
                        .find(|user| user.get_nickname().eq_ignore_ascii_case(&receiver))
                })
            };

            if channels
                .iter()
                .map(|channel| client.list_users(&channel))
                .map(find_receiver)
                .any(|option| option.is_some())
            {
                if !online.contains(&receiver) {
                    // online.push(receiver);
                }
                // TODO Change this when https://github.com/aatxe/irc/issues/136 gets resolved
                //continue;
            }

            let tm = time::now().to_timespec();
            let message = command.tokens[1..].join(" ");
            let tell = database::NewTellMessage {
                sender: &sender,
                receiver: &receiver.to_lowercase(),
                time: NaiveDateTime::from_timestamp(tm.sec, 0u32),
                message: &message,
            };

            debug!("Saving tell for {:?}", receiver);
            self.tells.write().insert_tell(&tell)?;
            no_receiver = false;
        }

        Ok(if no_receiver && online.is_empty() {
            String::from("Invalid receiver.")
        } else {
            match online.len() {
                0 => format!("Got it!"),
                1 => format!("{} is currently online.", online[0]),
                _ => format!("{} are currently online.", online.join(", ")),
            }
        })
    }

    fn on_namelist(&self, client: &C, channel: &str) -> Result<(), FrippyError> {
        let receivers = self
            .tells
            .read()
            .get_receivers()
            .context(FrippyErrorKind::Tell)?;

        if let Some(users) = client.list_users(channel) {
            debug!("Outstanding tells for {:?}", receivers);

            for receiver in users
                .iter()
                .map(|u| u.get_nickname())
                .filter(|u| receivers.iter().any(|r| r == &u.to_lowercase()))
            {
                self.send_tells(client, receiver)?;
            }

            Ok(())
        } else {
            Ok(())
        }
    }

    fn send_tells(&self, client: &C, receiver: &str) -> Result<(), FrippyError> {
        trace!("Checking {} for tells", receiver);

        if client.current_nickname() == receiver {
            return Ok(());
        }

        let mut tells = self.tells.write();

        let tell_messages = match tells.get_tells(&receiver.to_lowercase()) {
            Ok(t) => t,
            Err(e) => {
                // This warning only occurs if frippy is built without a database
                #[allow(unreachable_patterns)]
                return match e.kind() {
                    ErrorKind::NotFound => Ok(()),
                    _ => Err(e.context(FrippyErrorKind::Tell))?,
                };
            }
        };

        for tell in tell_messages {
            let now = Duration::new(time::now().to_timespec().sec as u64, 0);
            let dur = now - Duration::new(tell.time.timestamp() as u64, 0);
            let human_dur = format_duration(dur);

            let message = format!(
                "Tell from {} {} ago: {}",
                tell.sender, human_dur, tell.message
            );

            client
                .send_notice(receiver, &message)
                .context(FrippyErrorKind::Connection)?;

            debug!(
                "Sent {:?} from {:?} to {:?}",
                tell.message, tell.sender, receiver
            );
        }

        tells
            .delete_tells(&receiver.to_lowercase())
            .context(FrippyErrorKind::Tell)?;

        Ok(())
    }

    fn invalid_command(&self) -> &str {
        "Incorrect Command. \
         Send \"tell help\" for help."
    }

    fn help(&self) -> &str {
        "Used to send messages to offline users which they will receive when they come online.\r\n
         usage: tell user message\r\n\
         example: tell Foobar Hello!"
    }
}

impl<T: Database, C: FrippyClient> Plugin for Tell<T, C> {
    type Client = C;
    fn execute(&self, client: &Self::Client, message: &Message) -> ExecutionStatus {
        let res = match message.command {
            Command::JOIN(_, _, _) => self.send_tells(client, message.source_nickname().unwrap()),
            Command::NICK(ref nick) => self.send_tells(client, nick),
            Command::PRIVMSG(_, _) => self.send_tells(client, message.source_nickname().unwrap()),
            Command::Response(resp, ref chan_info, _) => {
                if resp == Response::RPL_NAMREPLY {
                    debug!("NAMREPLY info: {:?}", chan_info);

                    self.on_namelist(client, &chan_info[chan_info.len() - 1])
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };

        match res {
            Ok(_) => ExecutionStatus::Done,
            Err(e) => ExecutionStatus::Err(e),
        }
    }

    fn execute_threaded(&self, _: &Self::Client, _: &Message) -> Result<(), FrippyError> {
        panic!("Tell should not use threading")
    }

    fn command(&self, client: &Self::Client, command: PluginCommand) -> Result<(), FrippyError> {
        if command.tokens.is_empty() {
            client
                .send_notice(&command.source, &self.invalid_command())
                .context(FrippyErrorKind::Connection)?;
            return Ok(());
        }

        let sender = command.source.to_owned();

        match command.tokens[0].as_ref() {
            "help" => client
                .send_notice(&command.source, &self.help())
                .context(FrippyErrorKind::Connection),
            _ => match self.tell_command(client, command) {
                Ok(msg) => client
                    .send_notice(&sender, &msg)
                    .context(FrippyErrorKind::Connection),
                Err(e) => client
                    .send_notice(&sender, &e.to_string())
                    .context(FrippyErrorKind::Connection),
            },
        }?;

        Ok(())
    }

    fn evaluate(&self, _: &Self::Client, _: PluginCommand) -> Result<String, String> {
        Err(String::from("This Plugin does not implement any commands."))
    }
}

use std::fmt;
impl<T: Database, C: FrippyClient> fmt::Debug for Tell<T, C> {
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
