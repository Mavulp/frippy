use irc::client::prelude::*;
use irc::error::Error as IrcError;
use plugin::Plugin;

extern crate unicode_names;

register_plugin!(Emoji);

impl Emoji {
    fn emoji(&self, server: &IrcServer, content: &str, target: &str) -> Result<(), IrcError> {

        let mut names: Vec<String> = Vec::new();
        for emoji in self.return_emojis(&content) {

            names.push(match unicode_names::name(emoji) {
                Some(v) => format!("{}", v).to_lowercase(),
                None => format!("UNKNOWN"),
            });
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
            _ => Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
//    use ::tests::{make_server, get_server_value};
//
//    use irc::client::prelude::*;
//
//    use plugin::Plugin;
//    use super::Emoji;
//
//    #[test]
//    fn emoji_message() {
//        let     server = make_server("PRIVMSG test :😃\r\n");
//        let mut plugin  = Emoji::new();
//
//        server.for_each_incoming(|message| {
//            assert!(plugin.execute(&server, &message).is_ok());
//        }).unwrap();
//
//        assert_eq!("PRIVMSG test :smiling fce with open mouth\r\n", &*get_server_value(&server));
//    }
//
//    #[test]
//    fn no_emoji_message() {
//        let server = make_server("PRIVMSG test :test\r\n");
//        let mut plugin = Emoji::new();
//
//        server.for_each_incoming(|message| {
//            assert!(plugin.execute(&server, &message).is_ok());
//        }).unwrap();
//        assert_eq!("", &*get_server_value(&server));
//    }
}
