use antidote::RwLock;
use irc::client::prelude::*;
use std::thread::{sleep, spawn};
use std::{fmt, sync::Arc, time::Duration};

use chrono::{self, NaiveDateTime};
use time;

use plugin::*;
pub mod database;
mod parser;
use self::database::Database;
use self::parser::CommandParser;

use self::error::*;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::ResultExt;

fn get_time() -> NaiveDateTime {
    let tm = time::now().to_timespec();
    NaiveDateTime::from_timestamp(tm.sec, 0u32)
}

fn get_events<T: Database>(db: &RwLock<T>, in_next: chrono::Duration) -> Vec<database::Event> {
    loop {
        let before = get_time() + in_next;
        match db.read().get_events_before(&before) {
            Ok(events) => return events,
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    error!("Failed to get events: {}", e);
                }
            }
        }

        debug!("Sleeping for {:?}", in_next);
        sleep(in_next.to_std().expect("Failed to convert look ahead time"));
    }
}

fn run<T: Database>(client: &IrcClient, db: Arc<RwLock<T>>) {
    let look_ahead = chrono::Duration::minutes(2);

    let mut events = get_events(&db, look_ahead);

    let mut sleep_time = look_ahead
        .to_std()
        .expect("Failed to convert look ahead time");

    loop {
        let now = get_time();
        for event in events {
            if event.time <= now {
                let msg = format!("Reminder from {}: {}", event.author, event.content);
                if let Err(e) = client.send_notice(&event.receiver, &msg) {
                    error!("Failed to send reminder: {}", e);
                } else {
                    debug!("Sent reminder {:?}", event);

                    if let Some(repeat) = event.repeat {
                        let next_time = event.time + chrono::Duration::seconds(repeat as i64);

                        if let Err(e) = db.write().update_event_time(event.id, &next_time) {
                            error!("Failed to update reminder: {}", e);
                        } else {
                            debug!("Updated time on: {:?}", event);
                        }
                    } else if let Err(e) = db.write().delete_event(event.id) {
                        error!("Failed to delete reminder: {}", e);
                    }
                }
            } else {
                let until_event = (event.time - now)
                    .to_std()
                    .expect("Failed to convert until event time");

                if until_event < sleep_time {
                    sleep_time = until_event + Duration::from_secs(1);
                }
            }
        }

        debug!("Sleeping for {:?}", sleep_time);
        sleep(sleep_time);
        sleep_time = Duration::from_secs(120);

        events = get_events(&db, look_ahead);
    }
}

#[derive(PluginName)]
pub struct Remind<T: 'static + Database> {
    events: Arc<RwLock<T>>,
    has_reminder: RwLock<bool>,
}

impl<T: 'static + Database> Remind<T> {
    pub fn new(db: T) -> Self {
        let events = Arc::new(RwLock::new(db));

        Remind {
            events: events,
            has_reminder: RwLock::new(false),
        }
    }

    fn set(&self, command: PluginCommand) -> Result<&str, RemindError> {
        let parser = CommandParser::try_from_tokens(command.tokens)?;
        debug!("parser: {:?}", parser);

        let mut target = parser.get_target();
        if target == "me" {
            target = &command.source;
        }

        let event = database::NewEvent {
            receiver: target,
            content: &parser.get_message(),
            author: &command.source,
            time: &parser.get_time(Duration::from_secs(120))?,
            repeat: parser
                .get_repeat(Duration::from_secs(300))?
                .map(|d| d.as_secs()),
        };

        debug!("New event: {:?}", event);

        Ok(self.events.write().insert_event(&event).map(|()| "Got it")?)
    }

    fn list(&self, user: &str) -> Result<String, RemindError> {
        let mut events = self.events.read().get_user_events(user)?;

        let mut list = events.remove(0).to_string();
        for ev in events {
            list.push_str("\r\n");
            list.push_str(&ev.to_string());
        }

        Ok(list)
    }

    fn delete(&self, mut command: PluginCommand) -> Result<&str, RemindError> {
        let id = command
            .tokens
            .remove(0)
            .parse::<i64>()
            .context(ErrorKind::Parsing)?;
        let event = self.events.read().get_event(id)?;

        if event.receiver.eq_ignore_ascii_case(&command.source)
            || event.author.eq_ignore_ascii_case(&command.source)
        {
            self.events
                .write()
                .delete_event(id)
                .map(|()| "Successfully deleted")
        } else {
            Ok("Only the author or receiver can delete a reminder")
        }
    }

    fn help(&self) -> &str {
        "usage: remind <subcommand>\r\n\
         subcommands: new, list, delete, help"
    }
}

impl<T: Database> Plugin for Remind<T> {
    fn execute(&self, client: &IrcClient, _: &Message) -> ExecutionStatus {
        let mut has_reminder = self.has_reminder.write();
        if !*has_reminder {
            let events = Arc::clone(&self.events);
            let client = client.clone();

            spawn(move || run(&client, events));

            *has_reminder = true;
        }

        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), FrippyError> {
        panic!("Remind should not use frippy's threading")
    }

    fn command(&self, client: &IrcClient, mut command: PluginCommand) -> Result<(), FrippyError> {
        if command.tokens.is_empty() {
            return Ok(client
                .send_notice(&command.source, &ErrorKind::InvalidCommand.to_string())
                .context(FrippyErrorKind::Connection)?);
        }

        let source = command.source.clone();

        let sub_command = command.tokens.remove(0);
        let response = match sub_command.as_ref() {
            "new" => self.set(command).map(|s| s.to_owned()),
            "delete" => self.delete(command).map(|s| s.to_owned()),
            "list" => self.list(&source),
            "help" => Ok(self.help().to_owned()),
            _ => Err(ErrorKind::InvalidCommand.into()),
        };

        let result = match response {
            Ok(msg) => client
                .send_notice(&source, &msg)
                .context(FrippyErrorKind::Connection)?,
            Err(e) => {
                let message = e.to_string();

                client
                    .send_notice(&source, &message)
                    .context(FrippyErrorKind::Connection)?;

                Err(e).context(FrippyErrorKind::Remind)?
            }
        };

        Ok(result)
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from(
            "Evaluation of commands is not implemented for remind at this time",
        ))
    }
}

impl<T: Database> fmt::Debug for Remind<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Remind {{ ... }}")
    }
}

pub mod error {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "RemindError"]
    pub enum ErrorKind {
        /// Invalid command error
        #[fail(display = "Incorrect Command. Send \"currency help\" for help.")]
        InvalidCommand,

        /// Missing message error
        #[fail(display = "Reminder needs to have a description")]
        MissingMessage,

        /// Missing receiver error
        #[fail(display = "Specify who to remind")]
        MissingReceiver,

        /// Missing time error
        #[fail(display = "Reminder needs to have a time")]
        MissingTime,

        /// Invalid time error
        #[fail(display = "Could not parse time")]
        InvalidTime,

        /// Invalid date error
        #[fail(display = "Could not parse date")]
        InvalidDate,

        /// Parse error
        #[fail(display = "Could not parse integers")]
        Parsing,

        /// Ambigous time error
        #[fail(display = "Time specified is ambiguous")]
        AmbiguousTime,

        /// Time too short error
        #[fail(display = "Reminder needs to be in over 2 minutes")]
        TimeShort,

        /// Repeat time too short error
        #[fail(display = "Repeat time needs to be over 5 minutes")]
        RepeatTimeShort,

        /// Duplicate error
        #[fail(display = "Entry already exists")]
        Duplicate,

        /// Not found error
        #[fail(display = "No events found")]
        NotFound,
    }
}
