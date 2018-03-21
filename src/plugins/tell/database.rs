#[cfg(feature = "mysql")]
extern crate dotenv;

#[cfg(feature = "mysql")]
use std::sync::Arc;
use std::collections::HashMap;

#[cfg(feature = "mysql")]
use diesel::prelude::*;
#[cfg(feature = "mysql")]
use diesel::mysql::MysqlConnection;
#[cfg(feature = "mysql")]
use r2d2::Pool;
#[cfg(feature = "mysql")]
use r2d2_diesel::ConnectionManager;

use chrono::NaiveDateTime;

#[cfg(feature = "mysql")]
use failure::ResultExt;

use super::error::*;

#[cfg_attr(feature = "mysql", derive(Queryable))]
#[derive(PartialEq, Clone, Debug)]
pub struct TellMessage {
    pub id: i64,
    pub sender: String,
    pub receiver: String,
    pub time: NaiveDateTime,
    pub message: String,
}

#[cfg_attr(feature = "mysql", derive(Insertable))]
#[cfg_attr(feature = "mysql", table_name = "tells")]
pub struct NewTellMessage<'a> {
    pub sender: &'a str,
    pub receiver: &'a str,
    pub time: NaiveDateTime,
    pub message: &'a str,
}

pub trait Database: Send + Sync {
    fn insert_tell(&mut self, tell: &NewTellMessage) -> Result<(), TellError>;
    fn get_tells(&self, receiver: &str) -> Result<Vec<TellMessage>, TellError>;
    fn get_receivers(&self) -> Result<Vec<String>, TellError>;
    fn delete_tells(&mut self, receiver: &str) -> Result<(), TellError>;
}

// HashMap
impl Database for HashMap<String, Vec<TellMessage>> {
    fn insert_tell(&mut self, tell: &NewTellMessage) -> Result<(), TellError> {
        let tell = TellMessage {
            id: 0,
            sender: tell.sender.to_string(),
            receiver: tell.receiver.to_string(),
            time: tell.time,
            message: tell.message.to_string(),
        };

        let receiver = tell.receiver.clone();
        let tell_messages = self.entry(receiver)
            .or_insert_with(|| Vec::with_capacity(3));
        (*tell_messages).push(tell);

        Ok(())
    }

    fn get_tells(&self, receiver: &str) -> Result<Vec<TellMessage>, TellError> {
        Ok(self.get(receiver).cloned().ok_or(ErrorKind::NotFound)?)
    }

    fn get_receivers(&self) -> Result<Vec<String>, TellError> {
        Ok(self.iter()
            .map(|(receiver, _)| receiver.to_owned())
            .collect::<Vec<_>>())
    }

    fn delete_tells(&mut self, receiver: &str) -> Result<(), TellError> {
        match self.remove(receiver) {
            Some(_) => Ok(()),
            None => Err(ErrorKind::NotFound)?,
        }
    }
}

// Diesel automatically defines the tells module as public.
// We create a schema module to keep it private.
#[cfg(feature = "mysql")]
mod schema {
    table! {
        tells (id) {
            id -> Bigint,
            sender -> Varchar,
            receiver -> Varchar,
            time -> Timestamp,
            message -> Varchar,
        }
    }
}

#[cfg(feature = "mysql")]
use self::schema::tells;

#[cfg(feature = "mysql")]
impl Database for Arc<Pool<ConnectionManager<MysqlConnection>>> {
    fn insert_tell(&mut self, tell: &NewTellMessage) -> Result<(), TellError> {
        use diesel;

        let conn = &*self.get().expect("Failed to get connection");
        diesel::insert_into(tells::table)
            .values(tell)
            .execute(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(())
    }

    fn get_tells(&self, receiver: &str) -> Result<Vec<TellMessage>, TellError> {
        use self::tells::columns;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        Ok(tells::table
            .filter(columns::receiver.eq(receiver))
            .order(columns::time.asc())
            .load::<TellMessage>(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn get_receivers(&self) -> Result<Vec<String>, TellError> {
        use self::tells::columns;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        Ok(tells::table
            .select(columns::receiver)
            .load::<String>(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn delete_tells(&mut self, receiver: &str) -> Result<(), TellError> {
        use diesel;
        use self::tells::columns;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        diesel::delete(tells::table.filter(columns::receiver.eq(receiver)))
            .execute(conn)
            .context(ErrorKind::MysqlError)?;
        Ok(())
    }
}
