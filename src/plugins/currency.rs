extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::io::Read;
use std::num::ParseFloatError;

use irc::client::prelude::*;
use irc::error::IrcError;

use self::reqwest::Client;
use self::reqwest::header::Connection;
use self::serde_json::Value;

use plugin::*;

#[derive(PluginName, Default, Debug)]
pub struct Currency;

struct ConvertionRequest<'a> {
    value: f64,
    source: &'a str,
    target: &'a str,
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
                response.read_to_string(&mut body).ok()?;

                let convertion_rates: Result<Value, _> = serde_json::from_str(&body);
                match convertion_rates {
                    Ok(convertion_rates) => {
                        let rates: &Value = convertion_rates.get("rates")?;
                        let target_rate: &Value = rates.get(self.target.to_uppercase())?;
                        Some(self.value * target_rate.as_f64()?)
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

    fn eval_command<'a>(
        &self,
        tokens: &'a [String],
    ) -> Result<ConvertionRequest<'a>, ParseFloatError> {
        Ok(ConvertionRequest {
            value: tokens[0].parse()?,
            source: &tokens[1],
            target: &tokens[2],
        })
    }

    fn convert(&self, client: &IrcClient, command: &mut PluginCommand) -> Result<String, String> {
        if command.tokens.len() < 3 {
            return Err(self.invalid_command(client));
        }

        let request = match self.eval_command(&command.tokens) {
            Ok(request) => request,
            Err(_) => {
                return Err(self.invalid_command(client));
            }
        };

        match request.send() {
            Some(response) => {
                let response = format!(
                    "{} {} => {:.4} {}",
                    request.value,
                    request.source.to_lowercase(),
                    response / 1.00000000,
                    request.target.to_lowercase()
                );

                Ok(response)
            }
            None => Err(String::from(
                "An error occured during the conversion of the given currency",
            )),
        }
    }

    fn help(&self, client: &IrcClient) -> String {
        format!(
            "usage: {} currency value from_currency to_currency\r\n\
             example: {0} currency 1.5 eur usd\r\n\
             available currencies: AUD, BGN, BRL, CAD, \
             CHF, CNY, CZK, DKK, GBP, HKD, HRK, HUF, \
             IDR, ILS, INR, JPY, KRW, MXN, MYR, NOK, \
             NZD, PHP, PLN, RON, RUB, SEK, SGD, THB, \
             TRY, USD, ZAR",
            client.current_nickname()
        )
    }

    fn invalid_command(&self, client: &IrcClient) -> String {
        format!(
            "Incorrect Command. \
             Send \"{} currency help\" for help.",
            client.current_nickname()
        )
    }
}

impl Plugin for Currency {
    fn execute(&self, _: &IrcClient, _: &Message) -> ExecutionStatus {
        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), IrcError> {
        panic!("Currency does not implement the execute function!")
    }

    fn command(&self, client: &IrcClient, mut command: PluginCommand) -> Result<(), IrcError> {
        if command.tokens.is_empty() {
            return client.send_notice(&command.source, &self.invalid_command(client));
        }

        match command.tokens[0].as_ref() {
            "help" => client.send_notice(&command.source, &self.help(client)),
            _ => match self.convert(client, &mut command) {
                Ok(msg) => client.send_privmsg(&command.target, &msg),
                Err(msg) => client.send_notice(&command.source, &msg),
            },
        }
    }

    fn evaluate(&self, client: &IrcClient, mut command: PluginCommand) -> Result<String, String> {
        if command.tokens.is_empty() {
            return Err(self.invalid_command(client));
        }

        match command.tokens[0].as_ref() {
            "help" => Ok(self.help(client)),
            _ => self.convert(client, &mut command),
        }
    }
}

#[cfg(test)]
mod tests {}
