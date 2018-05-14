use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;

#[cfg(feature = "mysql")]
use std::sync::Arc;

#[cfg(feature = "mysql")]
use diesel::mysql::MysqlConnection;
#[cfg(feature = "mysql")]
use diesel::prelude::*;
#[cfg(feature = "mysql")]
use r2d2::Pool;
#[cfg(feature = "mysql")]
use r2d2_diesel::ConnectionManager;

#[cfg(feature = "mysql")]
use failure::ResultExt;

use chrono::NaiveDateTime;

use super::error::*;

#[cfg(feature = "mysql")]
static LAST_ID_SQL: &'static str = "SELECT LAST_INSERT_ID()";

#[cfg_attr(feature = "mysql", derive(Queryable))]
#[derive(Clone, Debug)]
pub struct Event {
    pub id: i64,
    pub receiver: String,
    pub content: String,
    pub author: String,
    pub time: NaiveDateTime,
    pub repeat: Option<i64>,
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

#[cfg_attr(feature = "mysql", derive(Insertable))]
#[cfg_attr(feature = "mysql", table_name = "events")]
#[derive(Debug)]
pub struct NewEvent<'a> {
    pub receiver: &'a str,
    pub content: &'a str,
    pub author: &'a str,
    pub time: &'a NaiveDateTime,
    pub repeat: Option<i64>,
}

pub trait Database: Send + Sync {
    fn insert_event(&mut self, event: &NewEvent) -> Result<i64, RemindError>;
    fn update_event_time(&mut self, id: i64, time: &NaiveDateTime) -> Result<(), RemindError>;
    fn get_events_before(&self, time: &NaiveDateTime) -> Result<Vec<Event>, RemindError>;
    fn get_user_events(&self, user: &str) -> Result<Vec<Event>, RemindError>;
    fn get_event(&self, id: i64) -> Result<Event, RemindError>;
    fn delete_event(&mut self, id: i64) -> Result<(), RemindError>;
}

// HashMap
impl Database for HashMap<i64, Event> {
    fn insert_event(&mut self, event: &NewEvent) -> Result<i64, RemindError> {
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
            None => Ok(id),
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

#[cfg(feature = "mysql")]
mod schema {
    table! {
        events (id) {
            id -> Bigint,
            receiver -> Varchar,
            content -> Text,
            author -> Varchar,
            time -> Timestamp,
            repeat -> Nullable<Bigint>,
        }
    }
}

#[cfg(feature = "mysql")]
use self::schema::events;

#[cfg(feature = "mysql")]
impl Database for Arc<Pool<ConnectionManager<MysqlConnection>>> {
    fn insert_event(&mut self, event: &NewEvent) -> Result<i64, RemindError> {
        use diesel::{self, dsl::sql, types::Bigint};
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        diesel::insert_into(events::table)
            .values(event)
            .execute(conn)
            .context(ErrorKind::MysqlError)?;

        let id = sql::<Bigint>(LAST_ID_SQL)
            .get_result(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(id)
    }

    fn update_event_time(&mut self, id: i64, time: &NaiveDateTime) -> Result<(), RemindError> {
        use self::events::columns;
        use diesel;
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        match diesel::update(events::table.filter(columns::id.eq(id)))
            .set(columns::time.eq(time))
            .execute(conn)
        {
            Ok(0) => Err(ErrorKind::NotFound)?,
            Ok(_) => Ok(()),
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }

    fn get_events_before(&self, time: &NaiveDateTime) -> Result<Vec<Event>, RemindError> {
        use self::events::columns;
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        Ok(events::table
            .filter(columns::time.lt(time))
            .load::<Event>(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn get_user_events(&self, user: &str) -> Result<Vec<Event>, RemindError> {
        use self::events::columns;
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        Ok(events::table
            .filter(columns::receiver.eq(user))
            .load::<Event>(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn get_event(&self, id: i64) -> Result<Event, RemindError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        Ok(events::table
            .find(id)
            .first(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn delete_event(&mut self, id: i64) -> Result<(), RemindError> {
        use self::events::columns;
        use diesel;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        match diesel::delete(events::table.filter(columns::id.eq(id))).execute(conn) {
            Ok(0) => Err(ErrorKind::NotFound)?,
            Ok(_) => Ok(()),
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }
}
