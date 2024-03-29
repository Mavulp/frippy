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
pub struct Quote {
    pub quotee: String,
    pub channel: String,
    pub idx: i32,
    pub content: String,
    pub author: String,
    pub created: NaiveDateTime,
}

#[cfg_attr(feature = "mysql", derive(Insertable))]
#[cfg_attr(feature = "mysql", table_name = "quotes")]
pub struct NewQuote<'a> {
    pub quotee: &'a str,
    pub channel: &'a str,
    pub idx: i32,
    pub content: &'a str,
    pub author: &'a str,
    pub created: NaiveDateTime,
}

pub trait Database: Send + Sync {
    fn insert_quote(&mut self, quote: &NewQuote) -> Result<(), QuoteError>;
    fn get_user_quote(&self, quotee: &str, channel: &str, idx: i32) -> Result<Quote, QuoteError>;
    fn get_channel_quote(&self, channel: &str, idx: i32) -> Result<Quote, QuoteError>;
    fn count_user_quotes(&self, quotee: &str, channel: &str) -> Result<i32, QuoteError>;
    fn count_channel_quotes(&self, channel: &str) -> Result<i32, QuoteError>;
    fn search_user_quote(
        &self,
        query: &str,
        quotee: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError>;

    fn search_channel_quote(
        &self,
        query: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError>;
}

// HashMap
impl<S: ::std::hash::BuildHasher + Send + Sync> Database
    for HashMap<(String, String, i32), Quote, S>
{
    fn insert_quote(&mut self, quote: &NewQuote) -> Result<(), QuoteError> {
        let quote = Quote {
            quotee: quote.quotee.to_owned(),
            channel: quote.channel.to_owned(),
            idx: quote.idx,
            content: quote.content.to_owned(),
            author: quote.author.to_owned(),
            created: quote.created,
        };

        let quotee = quote.quotee.clone();
        let channel = quote.channel.clone();
        match self.insert((quotee, channel, quote.idx), quote) {
            None => Ok(()),
            Some(_) => Err(ErrorKind::Duplicate)?,
        }
    }

    fn get_user_quote(&self, quotee: &str, channel: &str, idx: i32) -> Result<Quote, QuoteError> {
        let quote = self
            .get(&(quotee.to_owned(), channel.to_owned(), idx))
            .cloned()
            .ok_or(ErrorKind::NotFound)?;

        Ok(quote)
    }

    fn get_channel_quote(&self, channel: &str, idx: i32) -> Result<Quote, QuoteError> {
        let quote = self
            .iter()
            .filter(|&((_, c, _), _)| c == channel)
            .nth(idx as usize - 1)
            .ok_or(ErrorKind::NotFound)?
            .1
            .clone();

        Ok(quote)
    }

    fn count_user_quotes(&self, quotee: &str, channel: &str) -> Result<i32, QuoteError> {
        let count = self
            .iter()
            .filter(|&((q, c, _), _)| q == quotee && c == channel)
            .count();

        Ok(count as i32)
    }

    fn count_channel_quotes(&self, channel: &str) -> Result<i32, QuoteError> {
        let count = self.iter().filter(|&((_, c, _), _)| c == channel).count();

        Ok(count as i32)
    }

    fn search_user_quote(
        &self,
        query: &str,
        quotee: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError> {
        let quote = self
            .iter()
            .filter(|&((q, c, _), _)| q == quotee && c == channel)
            .filter(|&(&(_, _, _), q)| q.content.to_lowercase().contains(&query.to_lowercase()))
            .nth(offset as usize)
            .ok_or(ErrorKind::NotFound)?
            .1
            .clone();

        Ok(quote)
    }

    fn search_channel_quote(
        &self,
        query: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError> {
        let quote = self
            .iter()
            .filter(|&((_, c, _), _)| c == channel)
            .filter(|&(&(_, _, _), q)| q.content.contains(query))
            .nth(offset as usize)
            .ok_or(ErrorKind::NotFound)?
            .1
            .clone();

        Ok(quote)
    }
}

// Diesel automatically defines the quotes module as public.
// We create a schema module to keep it private.
#[cfg(feature = "mysql")]
mod schema {
    table! {
        quotes (quotee, channel, idx) {
            quotee -> Varchar,
            channel -> Varchar,
            idx -> Integer,
            content -> Text,
            author -> Varchar,
            created -> Timestamp,
        }
    }
}

#[cfg(feature = "mysql")]
use self::schema::quotes;

#[cfg(feature = "mysql")]
impl Database for Arc<Pool<ConnectionManager<MysqlConnection>>> {
    fn insert_quote(&mut self, quote: &NewQuote) -> Result<(), QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        diesel::insert_into(quotes::table)
            .values(quote)
            .execute(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(())
    }

    fn get_user_quote(&self, quotee: &str, channel: &str, idx: i32) -> Result<Quote, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let quote = quotes::table
            .find((quotee, channel, idx))
            .first(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(quote)
    }

    fn get_channel_quote(&self, channel: &str, idx: i32) -> Result<Quote, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let quote = quotes::table
            .filter(quotes::columns::channel.eq(channel))
            .offset(idx as i64 - 1)
            .first(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(quote)
    }

    fn count_user_quotes(&self, quotee: &str, channel: &str) -> Result<i32, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let count: Result<i64, _> = quotes::table
            .filter(quotes::columns::quotee.eq(quotee))
            .filter(quotes::columns::channel.eq(channel))
            .count()
            .get_result(conn);

        match count {
            Ok(c) => Ok(c as i32),
            Err(diesel::NotFound) => Ok(0),
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }

    fn count_channel_quotes(&self, channel: &str) -> Result<i32, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let count: Result<i64, _> = quotes::table
            .filter(quotes::columns::channel.eq(channel))
            .count()
            .get_result(conn);

        match count {
            Ok(c) => Ok(c as i32),
            Err(diesel::NotFound) => Ok(0),
            Err(e) => Err(e).context(ErrorKind::MysqlError)?,
        }
    }

    fn search_user_quote(
        &self,
        query: &str,
        quotee: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let quote = quotes::table
            .filter(quotes::columns::channel.eq(channel))
            .filter(quotes::columns::quotee.eq(quotee))
            .filter(quotes::columns::content.like(&format!("%{}%", query)))
            .offset(offset as i64)
            .first(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(quote)
    }

    fn search_channel_quote(
        &self,
        query: &str,
        channel: &str,
        offset: i32,
    ) -> Result<Quote, QuoteError> {
        let conn = &*self.get().context(ErrorKind::NoConnection)?;
        let quote = quotes::table
            .filter(quotes::columns::channel.eq(channel))
            .filter(quotes::columns::content.like(&format!("%{}%", query)))
            .offset(offset as i64)
            .first(conn)
            .context(ErrorKind::MysqlError)?;

        Ok(quote)
    }
}
