#[cfg(feature = "mysql")]
extern crate dotenv;

use std::collections::HashMap;
#[cfg(feature = "mysql")]
use std::sync::Arc;

#[cfg(feature = "mysql")]
use diesel::mysql::MysqlConnection;
#[cfg(feature = "mysql")]
use diesel::prelude::*;
#[cfg(feature = "mysql")]
use failure::ResultExt;
#[cfg(feature = "mysql")]
use r2d2::Pool;
#[cfg(feature = "mysql")]
use r2d2_diesel::ConnectionManager;

use chrono::NaiveDateTime;

use super::error::*;

#[cfg_attr(feature = "mysql", derive(Queryable))]
#[derive(Clone, Debug)]
pub struct Factoid {
    pub name: String,
    pub idx: i32,
    pub content: String,
    pub author: String,
    pub created: NaiveDateTime,
}

#[cfg_attr(feature = "mysql", derive(Insertable))]
#[cfg_attr(feature = "mysql", table_name = "factoids")]
pub struct NewFactoid<'a> {
    pub name: &'a str,
    pub idx: i32,
    pub content: &'a str,
    pub author: &'a str,
    pub created: NaiveDateTime,
}

pub trait Database: Send + Sync {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> Result<(), FactoidsError>;
    fn get_factoid(&self, name: &str, idx: i32) -> Result<Factoid, FactoidsError>;
    fn delete_factoid(&mut self, name: &str, idx: i32) -> Result<(), FactoidsError>;
    fn count_factoids(&self, name: &str) -> Result<i32, FactoidsError>;
}

// HashMap
impl Database for HashMap<(String, i32), Factoid> {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> Result<(), FactoidsError> {
        let factoid = Factoid {
            name: String::from(factoid.name),
            idx: factoid.idx,
            content: factoid.content.to_string(),
            author: factoid.author.to_string(),
            created: factoid.created,
        };

        let name = factoid.name.clone();
        match self.insert((name, factoid.idx), factoid) {
            None => Ok(()),
            Some(_) => Err(ErrorKind::Duplicate)?,
        }
    }

    fn get_factoid(&self, name: &str, idx: i32) -> Result<Factoid, FactoidsError> {
        Ok(self.get(&(String::from(name), idx))
            .cloned()
            .ok_or(ErrorKind::NotFound)?)
    }

    fn delete_factoid(&mut self, name: &str, idx: i32) -> Result<(), FactoidsError> {
        match self.remove(&(String::from(name), idx)) {
            Some(_) => Ok(()),
            None => Err(ErrorKind::NotFound)?,
        }
    }

    fn count_factoids(&self, name: &str) -> Result<i32, FactoidsError> {
        Ok(self.iter().filter(|&(&(ref n, _), _)| n == name).count() as i32)
    }
}

// Diesel automatically defines the factoids module as public.
// We create a schema module to keep it private.
#[cfg(feature = "mysql")]
mod schema {
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
use self::schema::factoids;

#[cfg(feature = "mysql")]
impl Database for Arc<Pool<ConnectionManager<MysqlConnection>>> {
    fn insert_factoid(&mut self, factoid: &NewFactoid) -> Result<(), FactoidsError> {
        use diesel;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        diesel::insert_into(factoids::table)
            .values(factoid)
            .execute(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(())
    }

    fn get_factoid(&self, name: &str, idx: i32) -> Result<Factoid, FactoidsError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        Ok(factoids::table
            .find((name, idx))
            .first(conn)
            .context(ErrorKind::MysqlError)?)
    }

    fn delete_factoid(&mut self, name: &str, idx: i32) -> Result<(), FactoidsError> {
        use self::factoids::columns;
        use diesel;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        match diesel::delete(
            factoids::table
                .filter(columns::name.eq(name))
                .filter(columns::idx.eq(idx)),
        ).execute(conn)
        {
            Ok(v) => {
                if v > 0 {
                    Ok(())
                } else {
                    Err(ErrorKind::NotFound)?
                }
            }
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }

    fn count_factoids(&self, name: &str) -> Result<i32, FactoidsError> {
        use diesel;

        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let count: Result<i64, _> = factoids::table
            .filter(factoids::columns::name.eq(name))
            .count()
            .get_result(conn);

        match count {
            Ok(c) => Ok(c as i32),
            Err(diesel::NotFound) => Ok(0),
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }
}
