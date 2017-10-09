extern crate unicode_names;

use irc::client::prelude::*;
use irc::error::Error as IrcError;
use plugin::Plugin;
use PluginCommand;

register_plugin!(Emoji);

impl Emoji {
    fn emoji(&self, server: &IrcServer, content: &str, target: &str) -> Result<(), IrcError> {

        let mut names: Vec<String> = Vec::new();
        for emoji in self.return_emojis(content) {

            let name = match unicode_names::name(emoji) {
                Some(v) => format!("{}", v).to_lowercase(),
                None => "UNKNOWN".to_string(),
            };

            names.push(name);
        }

        server.send_privmsg(target, &names.join(", "))
    }

    fn return_emojis(&self, string: &str) -> Vec<char> {

        let mut emojis: Vec<char> = Vec::new();

        for c in string.chars() {
            if self.is_emoji(&c) {
                emojis.push(c);
            }
        }

        emojis
    }

    fn is_emoji(&self, c: &char) -> bool {
        match *c { '\u{1F600}'...'\u{1F64F}'     // Emoticons
            | '\u{1F300}'...'\u{1F5FF}'          // Misc Symbols and Pictographs
            | '\u{1F680}'...'\u{1F6FF}'          // Transport and Map
            | '\u{2600}' ...'\u{26FF}'           // Misc symbols
            | '\u{2700}' ...'\u{27BF}'           // Dingbats
            | '\u{FE00}' ...'\u{FE0F}'           // Variation Selectors
            | '\u{1F900}'...'\u{1F9FF}'          // Supplemental Symbols and Pictographs
            | '\u{20D0}' ...'\u{20FF}' => true,  // Combining Diacritical Marks for Symbols
            _ => false,
        }
    }
}

impl Plugin for Emoji {
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
        match message.command {
            Command::PRIVMSG(_, _) => true,
            _ => false,
        }
    }

    fn execute(&mut self, server: &IrcServer, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::PRIVMSG(ref target, ref content) => self.emoji(server, content, target),
            _ => Ok(()),
        }
    }

    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }
}

#[cfg(test)]
mod tests {
}
