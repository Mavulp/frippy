extern crate htmlescape;
extern crate regex;

use irc::client::prelude::*;

use self::regex::Regex;

use plugin::*;
use utils;

use self::error::*;
use error::FrippyError;
use error::ErrorKind as FrippyErrorKind;
use failure::Fail;
use failure::ResultExt;

lazy_static! {
    static ref RE: Regex = Regex::new(r"(^|\s)(https?://\S+)").unwrap();
}

#[derive(PluginName, Debug)]
pub struct Url {
    max_kib: usize,
}

impl Url {
    /// If a file is larger than `max_kib` KiB the download is stopped
    pub fn new(max_kib: usize) -> Url {
        Url { max_kib: max_kib }
    }

    fn grep_url(&self, msg: &str) -> Option<String> {
        let captures = RE.captures(msg)?;
        debug!("Url captures: {:?}", captures);

        Some(captures.get(2)?.as_str().to_owned())
    }

    fn get_title<'a>(&self, body: &str) -> Result<String, UrlError> {
        let title = body.find("<title")
            .map(|tag| {
                body[tag..]
                    .find('>')
                    .map(|offset| tag + offset + 1)
                    .map(|start| {
                        body[start..]
                            .find("</title>")
                            .map(|offset| start + offset)
                            .map(|end| &body[start..end])
                    })
            })
            .and_then(|s| s.and_then(|s| s))
            .ok_or(ErrorKind::MissingTitle)?;

        debug!("Title: {:?}", title);

        htmlescape::decode_html(title).map_err(|_| ErrorKind::HtmlDecoding.into())
    }

    fn url(&self, text: &str) -> Result<String, UrlError> {
        let url = self.grep_url(text).ok_or(ErrorKind::MissingUrl)?;
        let body = utils::download(&url, Some(self.max_kib)).context(ErrorKind::Download)?;

        let title = self.get_title(&body)?;

        Ok(title.replace('\n', "|").replace('\r', "|"))
    }
}

impl Plugin for Url {
    fn execute(&self, _: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::PRIVMSG(_, ref msg) => if RE.is_match(msg) {
                ExecutionStatus::RequiresThread
            } else {
                ExecutionStatus::Done
            },
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, client: &IrcClient, message: &Message) -> Result<(), FrippyError> {
        Ok(match message.command {
            Command::PRIVMSG(_, ref content) => match self.url(content) {
                Ok(title) => client
                    .send_privmsg(message.response_target().unwrap(), &title)
                    .context(FrippyErrorKind::Connection)?,
                Err(e) => Err(e).context(FrippyErrorKind::Url)?,
            },
            _ => (),
        })
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), FrippyError> {
        Ok(client
            .send_notice(
                &command.source,
                "This Plugin does not implement any commands.",
            )
            .context(FrippyErrorKind::Connection)?)
    }

    fn evaluate(&self, _: &IrcClient, command: PluginCommand) -> Result<String, String> {
        self.url(&command.tokens[0])
            .map_err(|e| e.cause().unwrap().to_string())
    }
}

pub mod error {
    /// A URL plugin error
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "UrlError"]
    pub enum ErrorKind {
        /// A download error
        #[fail(display = "A download error occured")]
        Download,

        /// Missing URL error
        #[fail(display = "No URL was found")]
        MissingUrl,

        /// Missing title error
        #[fail(display = "No title was found")]
        MissingTitle,

        /// Html decoding error
        #[fail(display = "Failed to decode Html characters")]
        HtmlDecoding,
    }
}
