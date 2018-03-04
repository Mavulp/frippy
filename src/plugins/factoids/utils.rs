extern crate reqwest;

use std::thread;
use std::time::Duration;

use utils;
use super::rlua::prelude::*;

use self::LuaError::RuntimeError;

pub fn download(_: &Lua, url: String) -> Result<String, LuaError> {
    match utils::download(&url, Some(1024)) {
        Ok(v) => Ok(v),
        Err(e) => Err(RuntimeError(format!(
            "Failed to download {} - {}",
            url,
            e.to_string()
        ))),
    }
}

pub fn sleep(_: &Lua, dur: u64) -> Result<(), LuaError> {
    thread::sleep(Duration::from_millis(dur));
    Ok(())
}
