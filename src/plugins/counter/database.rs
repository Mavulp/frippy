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

use super::error::*;

pub trait Database: Send + Sync {
    fn add(&mut self, name: &str) -> Result<i64, CounterError>;
    fn subtract(&mut self, name: &str) -> Result<i64, CounterError>;
    fn get_count(&self, name: &str) -> Result<i64, CounterError>;
}

impl<S: ::std::hash::BuildHasher + Send + Sync> Database for HashMap<String, i64, S> {
    fn add(&mut self, name: &str) -> Result<i64, CounterError> {
        Ok(*self
            .entry(name.to_owned())
            .and_modify(|count| *count += 1)
            .or_insert(1))
    }
    fn subtract(&mut self, name: &str) -> Result<i64, CounterError> {
        Ok(*self
            .entry(name.to_owned())
            .and_modify(|count| *count -= 1)
            .or_insert(-1))
    }
    fn get_count(&self, name: &str) -> Result<i64, CounterError> {
        Ok(self.get(name).copied().unwrap_or(0))
    }
}

// Diesel automatically defines the counts module as public.
// We create a schema module to keep it private.
#[cfg(feature = "mysql")]
mod schema {
    diesel::table! {
        counts (name) {
            name -> Varchar,
            count -> Bigint,
        }
    }
}

#[cfg(feature = "mysql")]
use self::schema::counts;

#[cfg(feature = "mysql")]
impl Database for Arc<Pool<ConnectionManager<MysqlConnection>>> {
    fn add(&mut self, name: &str) -> Result<i64, CounterError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        match counts::table
            .find(name)
            .select(counts::columns::count)
            .first(conn)
        {
            Ok(mut count) => {
                count += 1;
                diesel::update(counts::table.filter(counts::columns::name.eq(name)))
                    .set(counts::columns::count.eq(count))
                    .execute(conn)
                    .context(ErrorKind::MysqlError)?;

                Ok(count)
            }
            Err(e) => match e {
                diesel::result::Error::NotFound => {
                    diesel::insert_into(counts::table)
                        .values((counts::columns::name.eq(name), counts::columns::count.eq(1)))
                        .execute(conn)
                        .context(ErrorKind::MysqlError)?;

                    Ok(1)
                }
                _ => Err(e).context(ErrorKind::MysqlError)?,
            },
        }
    }
    fn subtract(&mut self, name: &str) -> Result<i64, CounterError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        match counts::table
            .find(name)
            .select(counts::columns::count)
            .first(conn)
        {
            Ok(mut count) => {
                count -= 1;
                diesel::update(counts::table.filter(counts::columns::name.eq(name)))
                    .set(counts::columns::count.eq(count))
                    .execute(conn)
                    .context(ErrorKind::MysqlError)?;

                Ok(count)
            }
            Err(e) => match e {
                diesel::result::Error::NotFound => {
                    diesel::insert_into(counts::table)
                        .values((
                            counts::columns::name.eq(name),
                            counts::columns::count.eq(-1),
                        ))
                        .execute(conn)
                        .context(ErrorKind::MysqlError)?;

                    Ok(-1)
                }
                _ => Err(e).context(ErrorKind::MysqlError)?,
            },
        }
    }

    fn get_count(&self, name: &str) -> Result<i64, CounterError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;

        match counts::table
            .find(name)
            .select(counts::columns::count)
            .first(conn)
        {
            Ok(count) => Ok(count),
            Err(e) => match e {
                diesel::result::Error::NotFound => Ok(0),
                _ => Err(e).context(ErrorKind::MysqlError)?,
            },
        }
    }
}
