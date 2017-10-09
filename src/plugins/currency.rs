extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate regex;

use std::io::Read;
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use plugin::Plugin;
use self::reqwest::Client;
use self::reqwest::header::Connection;
use self::serde_json::Value;

use PluginCommand;

register_plugin!(Currency);

struct ConvertionRequest<'a> {
    value: f64,
    source: &'a str,
    target: &'a str,
}

macro_rules! try_option {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None    => { return None; }
        }
    }
}

impl<'a> ConvertionRequest<'a> {
    fn send(&self) -> Option<f64> {

        let response = Client::new()
            .get("https://api.fixer.io/latest")
            .form(&[("base", self.source)])
            .header(Connection::close())
            .send();

        match response {
            Ok(mut response) => {
                let mut body = String::new();
                try_option!(response.read_to_string(&mut body).ok());

                let convertion_rates: Result<Value, _> = serde_json::from_str(&body);
                match convertion_rates {
                    Ok(convertion_rates) => {

                        let rates: &Value = try_option!(convertion_rates.get("rates"));
                        let target_rate: &Value =
                            try_option!(rates.get(self.target.to_uppercase()));
                        Some(self.value * try_option!(target_rate.as_f64()))
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }
}

impl Currency {
    fn eval_command<'a>(&self, tokens: &'a [String]) -> Option<ConvertionRequest<'a>> {
        let parsed = match tokens[0].parse() {
            Ok(v) => v,
            Err(_) => {
                return None;
            }
        };

        Some(ConvertionRequest {
                 value: parsed,
                 source: &tokens[1],
                 target: &tokens[2],
             })
    }

    fn convert(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        let request = match self.eval_command(&command.tokens) {
            Some(request) => request,
            None => {
                return self.invalid_command(server, &command);
            }
        };

        match request.send() {
            Some(response) => {
                let response = format!("{} {} => {:.4} {}",
                                       request.value,
                                       request.source.to_lowercase(),
                                       response / 1.00000000,
                                       request.target.to_lowercase());

                server.send_privmsg(&command.target, &response)
            }
            None => server.send_notice(&command.source, "Error while converting given currency"),
        }
    }

    fn help(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        let usage = format!("usage: {} currency value from_currency to_currency",
                            server.current_nickname());

        if let Err(e) = server.send_notice(&command.source, &usage) {
            return Err(e);
        }
        server.send_notice(&command.source, "example: 1.5 eur usd")
    }

    fn invalid_command(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        let help = format!("Incorrect value. \
                           Send \"{} help currency\" for help.",
                           server.current_nickname());

        server.send_notice(&command.source, &help)
    }
}

impl Plugin for Currency {
    fn is_allowed(&self, _: &IrcServer, _: &Message) -> bool {
        false
    }

    fn execute(&mut self, _: &IrcServer, _: &Message) -> Result<(), IrcError> {
        Ok(())
    }

    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            self.invalid_command(server, &command)

        } else if command.tokens[0].to_lowercase() == "help" {
            self.help(server, command)

        } else if command.tokens.len() >= 3 {
            self.convert(server, command)

        } else {
            self.invalid_command(server, &command)
        }
    }
}

#[cfg(test)]
mod tests {
    //use tests::{make_server, get_server_value};

    //use irc::client::prelude::*;

    //use plugin::Plugin;
    //use regex::Regex;
    //use super::Currency;

    //#[test]
    //fn test_big_jpy_to_eur() {
    //    let server = make_server("PRIVMSG test :5000000 JPY to EUR\r\n");
    //    let mut plugin = Currency::new();

    //    for message in server.iter() {
    //        let message = message.unwrap();
    //        assert!(plugin.is_allowed(&server, &message));
    //        assert!(plugin.execute(&server, &message).is_ok());
    //    }

    //    let regex = Regex::new(r"=> ([0-9]{2})").unwrap();
    //    let msg = get_server_value(&server);
    //    let captures = regex.captures(&*msg).unwrap();
    //    assert!(captures.at(0).is_some());

    //    let result = captures
    //        .at(0)
    //        .unwrap()
    //        .split_whitespace()
    //        .last()
    //        .unwrap()
    //        .parse::<i32>();
    //    assert!(result.is_ok());

    //    let value = result.unwrap();
    //    assert!(value > 10 && value < 100);
    //}
}
