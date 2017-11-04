#[cfg(feature = "mysql")]
extern crate dotenv;

use std::collections::HashMap;

#[cfg(feature = "mysql")]
use diesel::prelude::*;

#[cfg(feature = "mysql")]
use diesel::mysql::MysqlConnection;

pub trait Database: Send {
    fn insert(&mut self, name: &str, content: &str) -> Option<()>;
    fn get(&self, name: &str) -> Option<String>;
}

impl Database for HashMap<String, String> {
    fn insert(&mut self, name: &str, content: &str) -> Option<()> {
        self.insert(String::from(name), String::from(content)).map(|_| ())
    }

    fn get(&self, name: &str) -> Option<String> {
        self.get(name).cloned()
    }
}

#[cfg(feature = "mysql")]
#[derive(Queryable)]
struct Factoid {
    pub name: String,
    pub content: String,
}

#[cfg(feature = "mysql")]
table! {
    factoids (name) {
        name -> Varchar,
        content -> Varchar,
    }
}

#[cfg(feature = "mysql")]
#[derive(Insertable)]
#[table_name="factoids"]
struct NewFactoid<'a> {
    pub name: &'a str,
    pub content: &'a str,
}


#[cfg(feature = "mysql")]
impl Database for MysqlConnection {
    fn insert(&mut self, name: &str, content: &str) -> Option<()> {
        let factoid = NewFactoid {
            name: name,
            content: content,
        };

        ::diesel::insert(&factoid)
            .into(factoids::table)
            .execute(self)
            .ok()
            .map(|_| ())
    }

    fn get(&self, name: &str) -> Option<String> {
        factoids::table
            .filter(factoids::columns::name.eq(name))
            .limit(1)
            .load::<Factoid>(self)
            .ok()
            .and_then(|v| v.first().map(|f| f.content.clone()))
    }
}
