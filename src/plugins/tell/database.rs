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

pub enum DbResponse {
    Success,
    Failed(&'static str),
}

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

pub trait Database: Send {
    fn insert_tell(&mut self, tell: &NewTellMessage) -> DbResponse;
    fn get_tells(&self, receiver: &str) -> Option<Vec<TellMessage>>;
    fn delete_tells(&mut self, receiver: &str) -> DbResponse;
}

// HashMap
impl Database for HashMap<String, Vec<TellMessage>> {
    fn insert_tell(&mut self, tell: &NewTellMessage) -> DbResponse {
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

        DbResponse::Success
    }

    fn get_tells(&self, receiver: &str) -> Option<Vec<TellMessage>> {
        self.get(receiver).cloned()
    }

    fn delete_tells(&mut self, receiver: &str) -> DbResponse {
        match self.remove(receiver) {
            Some(_) => DbResponse::Success,
            None => DbResponse::Failed("Tells not found"),
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
    fn insert_tell(&mut self, tell: &NewTellMessage) -> DbResponse {
        use diesel;

        let conn = &*self.get().expect("Failed to get connection");
        match diesel::insert_into(tells::table).values(tell).execute(conn) {
            Ok(_) => DbResponse::Success,
            Err(e) => {
                error!("DB failed to insert tell: {}", e);
                DbResponse::Failed("Failed to save Tell")
            }
        }
    }

    fn get_tells(&self, receiver: &str) -> Option<Vec<TellMessage>> {
        use self::tells::columns;

        let conn = &*self.get().expect("Failed to get connection");
        match tells::table
            .filter(columns::receiver.eq(receiver))
            .order(columns::time.asc())
            .load::<TellMessage>(conn)
        {
            Ok(f) => Some(f),
            Err(e) => {
                error!("DB failed to get tells: {}", e);
                None
            }
        }
    }

    fn delete_tells(&mut self, receiver: &str) -> DbResponse {
        use diesel;
        use self::tells::columns;

        let conn = &*self.get().expect("Failed to get connection");
        match diesel::delete(tells::table.filter(columns::receiver.eq(receiver))).execute(conn) {
            Ok(_) => DbResponse::Success,
            Err(e) => {
                error!("DB failed to delete tells: {}", e);
                DbResponse::Failed("Failed to delete tells")
            }
        }
    }
}
