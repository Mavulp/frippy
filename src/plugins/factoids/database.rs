#[cfg(feature = "mysql")]
extern crate dotenv;

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
#[derive(Clone, Debug)]
pub struct Factoid {
    pub name: String,
    pub idx: i32,
    pub content: String,
    pub author: String,
    pub created: NaiveDateTime,
}

#[cfg(feature = "mysql")]
use self::mysql::factoids;
#[cfg_attr(feature = "mysql", derive(Insertable))]
#[cfg_attr(feature = "mysql", table_name = "factoids")]
pub struct NewFactoid<'a> {
    pub name: &'a str,
    pub idx: i32,
    pub content: &'a str,
    pub author: &'a str,
    pub created: NaiveDateTime,
}

pub trait Database: Send {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> DbResponse;
    fn get_factoid(&self, name: &str, idx: i32) -> Option<Factoid>;
    fn delete_factoid(&mut self, name: &str, idx: i32) -> DbResponse;
    fn count_factoids(&self, name: &str) -> Result<i32, &'static str>;
}

// HashMap
impl Database for HashMap<(String, i32), Factoid> {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> DbResponse {
        let factoid = Factoid {
            name: String::from(factoid.name),
            idx: factoid.idx,
            content: factoid.content.to_string(),
            author: factoid.author.to_string(),
            created: factoid.created,
        };

        let name = factoid.name.clone();
        match self.insert((name, factoid.idx), factoid) {
            None => DbResponse::Success,
            Some(_) => DbResponse::Failed("Factoid was overwritten"),
        }
    }

    fn get_factoid(&self, name: &str, idx: i32) -> Option<Factoid> {
        self.get(&(String::from(name), idx)).cloned()
    }

    fn delete_factoid(&mut self, name: &str, idx: i32) -> DbResponse {
        match self.remove(&(String::from(name), idx)) {
            Some(_) => DbResponse::Success,
            None => DbResponse::Failed("Factoid not found"),
        }
    }

    fn count_factoids(&self, name: &str) -> Result<i32, &'static str> {
        Ok(self.iter().filter(|&(&(ref n, _), _)| n == name).count() as i32)
    }
}

// Diesel automatically define the factoids module as public.
// For now this is how we keep it private.
#[cfg(feature = "mysql")]
mod mysql {
    table! {
        factoids (name, idx) {
            name -> Varchar,
            idx -> Integer,
            content -> Text,
            author -> Varchar,
            created -> Timestamp,
        }
    }
}

#[cfg(feature = "mysql")]
impl Database for Pool<ConnectionManager<MysqlConnection>> {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> DbResponse {
        use diesel;

        let conn = &*self.get().expect("Failed to get connection");
        match diesel::insert_into(factoids::table)
            .values(factoid)
            .execute(conn)
        {
            Ok(_) => DbResponse::Success,
            Err(e) => {
                error!("DB Insertion Error: {}", e);
                DbResponse::Failed("Failed to add factoid")
            }
        }
    }

    fn get_factoid(&self, name: &str, idx: i32) -> Option<Factoid> {
        let conn = &*self.get().expect("Failed to get connection");
        match factoids::table.find((name, idx)).first(conn) {
            Ok(f) => Some(f),
            Err(e) => {
                error!("DB Count Error: {}", e);
                None
            }
        }
    }

    fn delete_factoid(&mut self, name: &str, idx: i32) -> DbResponse {
        use diesel;
        use self::factoids::columns;

        let conn = &*self.get().expect("Failed to get connection");
        match diesel::delete(
            factoids::table
                .filter(columns::name.eq(name))
                .filter(columns::idx.eq(idx)),
        ).execute(conn)
        {
            Ok(v) => {
                if v > 0 {
                    DbResponse::Success
                } else {
                    DbResponse::Failed("Could not find any factoid with that name")
                }
            }
            Err(e) => {
                error!("DB Deletion Error: {}", e);
                DbResponse::Failed("Failed to delete factoid")
            }
        }
    }

    fn count_factoids(&self, name: &str) -> Result<i32, &'static str> {
        use diesel;

        let conn = &*self.get().expect("Failed to get connection");
        let count: Result<i64, _> = factoids::table
            .filter(factoids::columns::name.eq(name))
            .count()
            .get_result(conn);

        match count {
            Ok(c) => Ok(c as i32),
            Err(diesel::NotFound) => Ok(0),
            Err(e) => {
                error!("DB Count Error: {}", e);
                Err("Database Error")
            }
        }
    }
}
