extern crate reqwest;

use std::thread;
use std::time::Duration;

use super::rlua::prelude::*;
use utils::Url;

use self::LuaError::RuntimeError;

pub fn download(_: &Lua, url: String) -> Result<String, LuaError> {
    let url = Url::from(url).max_kib(1024);
    match url.request() {
        Ok(v) => Ok(v),
        Err(e) => Err(RuntimeError(format!(
            "Failed to download {} - {}",
            url.as_str(),
            e.to_string()
        ))),
    }
}

pub fn sleep(_: &Lua, dur: u64) -> Result<(), LuaError> {
    thread::sleep(Duration::from_millis(dur));
    Ok(())
}
