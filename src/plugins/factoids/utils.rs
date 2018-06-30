use std::thread;
use std::time::Duration;

use serde_json::{self, Value as SerdeValue};

use super::rlua::Error as LuaError;
use super::rlua::Error::RuntimeError;
use super::rlua::{Lua, Value as LuaValue};

use utils::error::ErrorKind::Connection;
use utils::Url;

use failure::Fail;

pub fn sleep(_: &Lua, dur: u64) -> Result<(), LuaError> {
    thread::sleep(Duration::from_millis(dur));
    Ok(())
}

pub fn download(_: &Lua, url: String) -> Result<String, LuaError> {
    let url = Url::from(url).max_kib(1024);
    match url.request() {
        Ok(v) => Ok(v),
        Err(e) => {
            let error = match e.kind() {
                Connection => e.cause().unwrap().to_string(),
                _ => e.to_string(),
            };

            Err(RuntimeError(format!(
                "Failed to download {} - {}",
                url.as_str(),
                error
            )))
        }
    }
}

fn convert_value(lua: &Lua, sval: SerdeValue, max_recurs: usize) -> Result<LuaValue, LuaError> {
    if max_recurs == 0 {
        return Err(RuntimeError(String::from(
            "Reached max recursion level - json is nested too deep",
        )));
    }

    let lval = match sval {
        SerdeValue::Null => LuaValue::Nil,
        SerdeValue::Bool(b) => LuaValue::Boolean(b),
        SerdeValue::String(s) => LuaValue::String(lua.create_string(&s)?),
        SerdeValue::Number(n) => {
            let f = n.as_f64().ok_or(RuntimeError(String::from(
                "Failed to convert number into double",
            )))?;
            LuaValue::Number(f)
        }
        SerdeValue::Array(arr) => {
            let table = lua.create_table()?;
            for (i, val) in arr.into_iter().enumerate() {
                table.set(i + 1, convert_value(lua, val, max_recurs - 1)?)?;
            }

            LuaValue::Table(table)
        }
        SerdeValue::Object(obj) => {
            let table = lua.create_table()?;
            for (key, val) in obj {
                table.set(key, convert_value(lua, val, max_recurs - 1)?)?;
            }

            LuaValue::Table(table)
        }
    };

    Ok(lval)
}

pub fn json_decode(lua: &Lua, json: String) -> Result<LuaValue, LuaError> {
    let ser_val: SerdeValue = serde_json::from_str(&json).map_err(|e| RuntimeError(e.to_string()))?;

    convert_value(lua, ser_val, 25)
}
