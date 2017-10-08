use std::fmt;
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use PluginCommand;

pub trait Plugin: Send + Sync + fmt::Display + fmt::Debug {
    fn is_allowed(&self, server: &IrcServer, message: &Message) -> bool;
    fn execute(&mut self, server: &IrcServer, message: &Message) -> Result<(), IrcError>;
    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError>;
}

#[macro_export]
macro_rules! register_plugin {
    ($t:ident) => {
        use std::fmt;

        #[derive(Debug)]
        pub struct $t {
            _name: &'static str,
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", self._name)
            }
        }

        impl $t {
            pub fn new() -> $t {
                $t { _name: stringify!($t) }
            }
        }
    };
}
