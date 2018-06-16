extern crate reqwest;

use std::thread;
use std::time::Duration;

use super::rlua::Error as LuaError;
use super::rlua::Lua;
use utils::error::ErrorKind::Connection;
use utils::Url;

use failure::Fail;

pub fn download(_: &Lua, url: String) -> Result<String, LuaError> {
    let url = Url::from(url).max_kib(1024);
    match url.request() {
        Ok(v) => Ok(v),
        Err(e) => {
            let error = match e.kind() {
                Connection => e.cause().unwrap().to_string(),
                _ => e.to_string(),
            };

            Err(LuaError::RuntimeError(format!(
                "Failed to download {} - {}",
                url.as_str(),
                error
            )))
        }
    }
}

pub fn sleep(_: &Lua, dur: u64) -> Result<(), LuaError> {
    thread::sleep(Duration::from_millis(dur));
    Ok(())
}
