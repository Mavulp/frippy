extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate regex;

use std::io::Read;
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use self::regex::Regex;
use plugin::Plugin;
use self::reqwest::Client;
use self::reqwest::header::Connection;
use self::serde_json::Value;

register_plugin!(Currency);

lazy_static! {
    static ref RE: Regex = Regex::new(r"([0-9]+) ([A-Za-z]+) (?i)(to) ([A-Za-z]+)").unwrap();
}

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
                println!("{}", body);

                let convertion_rates: Result<Value, _> = serde_json::from_str(&body);
                match convertion_rates {
                    Ok(convertion_rates) => {

                        let rates: &Value = try_option!(convertion_rates.get("rates"));
                        let target_rate: &Value = try_option!(rates.get(self.target.to_uppercase()));
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
    fn grep_request<'a>(&self, msg: &'a str) -> Option<ConvertionRequest<'a>> {
        match RE.captures(msg) {
            Some(captures) => {
                Some(ConvertionRequest {
                         value: {
                             let capture = try_option!(captures.get(1)).as_str();
                             try_option!(capture.parse().ok())
                         },
                         source: try_option!(captures.get(2)).as_str(),
                         target: try_option!(captures.get(4)).as_str(), // 3 is to/TO
                     })
            }
            None => None,
        }
    }

    fn convert(&self,
               server: &IrcServer,
               _: &Message,
               target: &str,
               msg: &str)
               -> Result<(), IrcError> {
        let request = match self.grep_request(msg) {
            Some(request) => request,
            None => {
                return Ok(());
            }
        };

        match request.send() {
            Some(response) => {
                server.send_privmsg(target,
                                    &*format!("{} {} => {:.4} {}",
                                              request.value,
                                              request.source,
                                              response / 1.00000000,
                                              request.target))
            }
            None => server.send_privmsg(target, "Error while converting given currency"),
        }
    }
}

impl Plugin for Currency {
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
        match message.command {
            Command::PRIVMSG(_, ref msg) => RE.is_match(msg),
            _ => false,
        }
    }

    fn execute(&mut self, server: &IrcServer, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => self.convert(server, message, target, msg),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use tests::{make_server, get_server_value};

    use irc::client::prelude::*;

    use plugin::Plugin;
    use regex::Regex;
    use super::Currency;

    #[test]
    fn test_big_jpy_to_eur() {
        let server = make_server("PRIVMSG test :5000000 JPY to EUR\r\n");
        let mut plugin = Currency::new();

        for message in server.iter() {
            let message = message.unwrap();
            assert!(plugin.is_allowed(&server, &message));
            assert!(plugin.execute(&server, &message).is_ok());
        }

        let regex = Regex::new(r"=> ([0-9]{2})").unwrap();
        let msg = get_server_value(&server);
        let captures = regex.captures(&*msg).unwrap();
        assert!(captures.at(0).is_some());

        let result = captures
            .at(0)
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<i32>();
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value > 10 && value < 100);
    }
}
