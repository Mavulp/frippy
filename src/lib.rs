#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate irc;
#[macro_use]
extern crate lazy_static;

#[macro_use]
mod plugin;
mod plugins;

use std::thread::spawn;
use std::sync::{Arc, Mutex};
use irc::client::prelude::*;

use plugin::Plugin;

pub fn run() {
    let server = IrcServer::new("config.toml").unwrap();
    server.identify().unwrap();

    let plugins: Vec<Arc<Mutex<Plugin>>> =
        vec![Arc::new(Mutex::new(plugins::emoji::Emoji::new())),
             Arc::new(Mutex::new(plugins::currency::Currency::new()))];

    server
        .for_each_incoming(|message| {
            let message = Arc::new(message);

            for plugin in plugins.clone() {
                let server = server.clone();
                let message = Arc::clone(&message);

                spawn(move || {
                    let mut plugin = match plugin.lock() {
                        Ok(plugin) => plugin,
                        Err(poisoned) => poisoned.into_inner(),
                    };

                    if plugin.is_allowed(&server, &message) {
                        plugin.execute(&server, &message).unwrap();
                    }
                });
            }
        })
        .unwrap();
}

#[cfg(test)]
mod tests {
    use irc::client::prelude::*;

    pub fn make_server(cmd: &str) -> IrcServer {
        let config = Config {
            nickname: Some(format!("test")),
            server: Some(format!("irc.test.net")),
            channels: Some(vec![format!("#test")]),
            use_mock_connection: Some(true),
            ..Default::default()
        };

        IrcServer::from_config(config).unwrap()
    }

    pub fn get_server_value(server: &IrcServer) -> String {
        unimplemented!();
    }
}
