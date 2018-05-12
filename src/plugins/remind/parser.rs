use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use humantime::parse_duration;
use std::time::Duration;
use time;

use super::error::*;
use failure::ResultExt;

#[derive(Default, Debug)]
pub struct CommandParser {
    on_date: Option<String>,
    at_time: Option<String>,
    in_duration: Option<String>,
    every_time: Option<String>,
    target: String,
    message: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum ParseState {
    None,
    On,
    At,
    In,
    Every,
    Msg,
}

impl CommandParser {
    pub fn try_from_tokens(tokens: Vec<String>) -> Result<Self, RemindError> {
        if tokens.is_empty() {
            return Err(ErrorKind::MissingReceiver.into());
        }

        let mut parser = CommandParser::default();
        let mut state = ParseState::None;

        let mut iter = tokens.into_iter();
        parser.target = iter.next()
            .expect("This should be guaranteed by the length check");

        let mut cur_str = String::new();
        while let Some(token) = iter.next() {
            let next_state = match token.as_ref() {
                "on" => ParseState::On,
                "at" => ParseState::At,
                "in" => ParseState::In,
                "every" => ParseState::Every,
                "to" => ParseState::Msg,
                _ => {
                    if !cur_str.is_empty() {
                        cur_str.push(' ');
                    }
                    cur_str.push_str(&token);
                    state
                }
            };

            if next_state != state {
                if state != ParseState::None {
                    parser = parser.add_string_by_state(&state, cur_str)?;
                    cur_str = String::new();
                }

                state = next_state;
            }
        }
        parser = parser.add_string_by_state(&state, cur_str)?;

        if parser.message.is_none() {
            return Err(ErrorKind::MissingMessage.into());
        }

        if parser.in_duration.is_some() && parser.at_time.is_some()
            || parser.in_duration.is_some() && parser.on_date.is_some()
        {
            return Err(ErrorKind::AmbiguousTime.into());
        }

        if parser.in_duration.is_none() && parser.at_time.is_none() && parser.on_date.is_none() {
            return Err(ErrorKind::MissingTime.into());
        }

        Ok(parser)
    }

    fn add_string_by_state(self, state: &ParseState, string: String) -> Result<Self, RemindError> {
        use self::ParseState::*;
        let string = Some(string);
        match state {
            &On if self.on_date.is_none() => {
                return Ok(CommandParser {
                    on_date: string,
                    ..self
                })
            }
            &At if self.at_time.is_none() => {
                return Ok(CommandParser {
                    at_time: string,
                    ..self
                })
            }
            &In if self.in_duration.is_none() => {
                return Ok(CommandParser {
                    in_duration: string,
                    ..self
                })
            }
            &Msg if self.message.is_none() => {
                return Ok(CommandParser {
                    message: string,
                    ..self
                })
            }
            &Every if self.every_time.is_none() => {
                return Ok(CommandParser {
                    every_time: string,
                    ..self
                })
            }
            _ => Err(ErrorKind::MissingMessage.into()),
        }
    }

    fn parse_date(&self, str_date: &str) -> Result<NaiveDate, RemindError> {
        let nums = str_date
            .split('.')
            .map(|s| s.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()
            .context(ErrorKind::InvalidDate)?;

        if 2 > nums.len() || nums.len() > 3 {
            return Err(ErrorKind::InvalidDate.into());
        }

        let day = nums[0];
        let month = nums[1];

        let parse_date = match nums.get(2) {
            Some(year) => {
                NaiveDate::from_ymd_opt(*year as i32, month, day).ok_or(ErrorKind::InvalidDate)?
            }
            None => {
                let now = time::now();
                let date = NaiveDate::from_ymd_opt(now.tm_year + 1900, month, day)
                    .ok_or(ErrorKind::InvalidDate)?;
                if date.succ().and_hms(0, 0, 0).timestamp() < now.to_timespec().sec {
                    NaiveDate::from_ymd(now.tm_year + 1901, month, day)
                } else {
                    date
                }
            }
        };

        Ok(parse_date)
    }

    fn parse_time(&self, str_time: &str) -> Result<NaiveTime, RemindError> {
        let nums = str_time
            .split(':')
            .map(|s| s.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()
            .context(ErrorKind::InvalidTime)?;

        if 2 != nums.len() {
            return Err(ErrorKind::InvalidTime.into());
        }

        let hour = nums[0];
        let minute = nums[1];

        Ok(NaiveTime::from_hms(hour, minute, 0))
    }

    pub fn get_time(&self, min_dur: Duration) -> Result<NaiveDateTime, RemindError> {
        if let Some(ref str_duration) = self.in_duration {
            let duration = parse_duration(&str_duration).context(ErrorKind::InvalidTime)?;

            if duration < min_dur {
                return Err(ErrorKind::TimeShort.into());
            }

            let tm = time::now().to_timespec();
            return Ok(NaiveDateTime::from_timestamp(
                tm.sec + duration.as_secs() as i64,
                0u32,
            ));
        }

        let mut date = None;
        if let Some(ref str_date) = self.on_date {
            date = Some(self.parse_date(str_date)?);
        }

        if let Some(ref str_time) = self.at_time {
            let time = self.parse_time(str_time)?;

            if let Some(date) = date {
                Ok(date.and_time(time))
            } else {
                let now = time::now();
                let today = NaiveDate::from_ymd_opt(
                    now.tm_year + 1900,
                    now.tm_mon as u32 + 1,
                    now.tm_mday as u32,
                ).ok_or(ErrorKind::InvalidDate)?;

                let time_today = today.and_time(time);

                if time_today.timestamp() < now.to_timespec().sec {
                    debug!("tomorrow");

                    Ok(today.succ().and_time(time))
                } else {
                    debug!("today");

                    Ok(time_today)
                }
            }
        } else {
            Ok(date.expect("At this point date has to be set")
                .and_hms(0, 0, 0))
        }
    }

    pub fn get_repeat(&self, min_dur: Duration) -> Result<Option<Duration>, RemindError> {
        if let Some(mut words) = self.every_time.clone() {
            if !words.chars().next().unwrap().is_digit(10) {
                words.insert(0, '1');
            }
            let dur = parse_duration(&words).context(ErrorKind::InvalidTime)?;

            if dur < min_dur {
                return Err(ErrorKind::RepeatTimeShort.into());
            }

            Ok(Some(dur))
        } else {
            Ok(None)
        }
    }

    pub fn get_target(&self) -> &str {
        &self.target
    }

    pub fn get_message(&self) -> &str {
        self.message.as_ref().expect("Has to be set")
    }
}
