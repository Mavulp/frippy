extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate regex;

use std::io::Read;
use std::num::ParseFloatError;

use irc::client::prelude::*;
use irc::error::Error as IrcError;

use self::reqwest::Client;
use self::reqwest::header::Connection;
use self::serde_json::Value;

use plugin::*;

#[derive(PluginName, Debug)]
pub struct Currency;

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
    pub fn new() -> Currency {
        Currency {}
    }

    fn eval_command<'a>(&self, tokens: &'a [String]) -> Result<ConvertionRequest<'a>, ParseFloatError> {
        Ok(ConvertionRequest {
            value: tokens[0].parse()?,
            source: &tokens[1],
            target: &tokens[2],
        })
    }

    fn convert(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {

        if command.tokens.len() < 3 {
            return self.invalid_command(server, &command);
        }

        let request = match self.eval_command(&command.tokens) {
            Ok(request) => request,
            Err(_) => {
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

    fn help(&self, server: &IrcServer, command: &mut PluginCommand) -> Result<(), IrcError> {
        let help = format!("usage: {} currency value from_currency to_currency\r\n\
                            example: 1.5 eur usd\r\n\
                            available currencies: AUD, BGN, BRL, CAD, \
                            CHF, CNY, CZK, DKK, GBP, HKD, HRK, HUF, \
                            IDR, ILS, INR, JPY, KRW, MXN, MYR, NOK, \
                            NZD, PHP, PLN, RON, RUB, SEK, SGD, THB, \
                            TRY, USD, ZAR",
                           server.current_nickname());

        server.send_notice(&command.source, &help)
    }

    fn invalid_command(&self, server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
        let help = format!("Incorrect Command. \
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

    fn command(&mut self, server: &IrcServer, mut command: PluginCommand) -> Result<(), IrcError> {

        if command.tokens.is_empty() {
            return self.invalid_command(server, &command);
        }

        match command.tokens[0].as_ref() {
            "help" => self.help(server, &mut command),
            _ => self.convert(server, command),
        }
    }
}

#[cfg(test)]
mod tests {}
