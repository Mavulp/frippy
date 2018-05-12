#[cfg(feature = "mysql")]
extern crate dotenv;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;

use chrono::NaiveDateTime;

use super::error::*;

#[derive(Clone, Debug)]
pub struct Event {
    pub id: i64,
    pub receiver: String,
    pub content: String,
    pub author: String,
    pub time: NaiveDateTime,
    pub repeat: Option<u64>,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: {} reminds {} to \"{}\" at {}",
            self.id, self.author, self.receiver, self.content, self.time
        )
    }
}

#[derive(Debug)]
pub struct NewEvent<'a> {
    pub receiver: &'a str,
    pub content: &'a str,
    pub author: &'a str,
    pub time: &'a NaiveDateTime,
    pub repeat: Option<u64>,
}

pub trait Database: Send + Sync {
    fn insert_event(&mut self, event: &NewEvent) -> Result<(), RemindError>;
    fn update_event_time(&mut self, id: i64, &NaiveDateTime) -> Result<(), RemindError>;
    fn get_events_before(&self, time: &NaiveDateTime) -> Result<Vec<Event>, RemindError>;
    fn get_user_events(&self, user: &str) -> Result<Vec<Event>, RemindError>;
    fn get_event(&self, id: i64) -> Result<Event, RemindError>;
    fn delete_event(&mut self, id: i64) -> Result<(), RemindError>;
}

// HashMap
impl Database for HashMap<i64, Event> {
    fn insert_event(&mut self, event: &NewEvent) -> Result<(), RemindError> {
        let mut id = 0;
        while self.contains_key(&id) {
            id += 1;
        }

        let event = Event {
            id: id,
            receiver: event.receiver.to_owned(),
            content: event.content.to_owned(),
            author: event.author.to_owned(),
            time: event.time.clone(),
            repeat: event.repeat,
        };

        match self.insert(id, event) {
            None => Ok(()),
            Some(_) => Err(ErrorKind::Duplicate)?,
        }
    }

    fn update_event_time(&mut self, id: i64, time: &NaiveDateTime) -> Result<(), RemindError> {
        let entry = self.entry(id);

        match entry {
            Entry::Occupied(mut v) => v.get_mut().time = *time,
            Entry::Vacant(_) => return Err(ErrorKind::NotFound.into()),
        }

        Ok(())
    }

    fn get_events_before(&self, time: &NaiveDateTime) -> Result<Vec<Event>, RemindError> {
        let mut events = Vec::new();

        for (_, event) in self.iter() {
            if &event.time < time {
                events.push(event.clone())
            }
        }

        if events.is_empty() {
            Err(ErrorKind::NotFound.into())
        } else {
            Ok(events)
        }
    }

    fn get_user_events(&self, user: &str) -> Result<Vec<Event>, RemindError> {
        let mut events = Vec::new();

        for (_, event) in self.iter() {
            if event.receiver.eq_ignore_ascii_case(user) {
                events.push(event.clone())
            }
        }

        if events.is_empty() {
            Err(ErrorKind::NotFound.into())
        } else {
            Ok(events)
        }
    }

    fn get_event(&self, id: i64) -> Result<Event, RemindError> {
        Ok(self.get(&id)
            .map(|ev| ev.clone())
            .ok_or(ErrorKind::NotFound)?)
    }

    fn delete_event(&mut self, id: i64) -> Result<(), RemindError> {
        match self.remove(&id) {
            Some(_) => Ok(()),
            None => Err(ErrorKind::NotFound)?,
        }
    }
}
