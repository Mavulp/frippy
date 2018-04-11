extern crate htmlescape;

use irc::client::prelude::*;

use regex::Regex;

use plugin::*;
use utils::Url;

use self::error::*;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::Fail;
use failure::ResultExt;

lazy_static! {
    static ref URL_RE: Regex = Regex::new(r"(^|\s)(https?://\S+)").unwrap();
    static ref WORD_RE: Regex = Regex::new(r"(\w+)").unwrap();
}

#[derive(PluginName, Debug)]
pub struct UrlTitles {
    max_kib: usize,
}

#[derive(Clone, Debug)]
struct Title(String);

impl From<String> for Title {
    fn from(title: String) -> Self {
        Title(title)
    }
}

impl From<Title> for String {
    fn from(title: Title) -> Self {
        title.0
    }
}

impl Title {
    fn find_by_delimiters(body: &str, delimiters: [&str; 3]) -> Result<Self, UrlError> {
        let title = body.find(delimiters[0])
            .map(|tag| {
                body[tag..]
                    .find(delimiters[1])
                    .map(|offset| tag + offset + delimiters[1].len())
                    .map(|start| {
                        body[start..]
                            .find(delimiters[2])
                            .map(|offset| start + offset)
                            .map(|end| &body[start..end])
                    })
            })
            .and_then(|s| s.and_then(|s| s))
            .ok_or(ErrorKind::MissingTitle)?;

        debug!("delimiters: {:?}", delimiters);
        debug!("title: {:?}", title);

        htmlescape::decode_html(title)
            .map(|t| t.into())
            .map_err(|_| ErrorKind::HtmlDecoding.into())
    }

    fn find_ogtitle<'a>(body: &str) -> Result<Self, UrlError> {
        Self::find_by_delimiters(body, ["property=\"og:title\"", "content=\"", "\""])
    }

    fn find_title<'a>(body: &str) -> Result<Self, UrlError> {
        Self::find_by_delimiters(body, ["<title", ">", "</title>"])
    }

    // TODO Improve logic
    pub fn usefulness(&self, url: &str) -> usize {
        let mut usefulness = 0;
        for word in WORD_RE.find_iter(&self.0) {
            let w = word.as_str().to_lowercase();
            if w.len() > 2 && !url.to_lowercase().contains(&w) {
                usefulness += 1;
            }
        }

        usefulness
    }

    fn clean_up(self) -> Self {
        self.0.trim().replace('\n', "|").replace('\r', "|").into()
    }

    pub fn find_clean_ogtitle<'a>(body: &str, url: &str) -> Result<Self, UrlError> {
        Self::find_ogtitle(body)
            .map(|t| t.clean_up())
    }

    pub fn find_clean_title<'a>(body: &str, url: &str) -> Result<Self, UrlError> {
        Self::find_title(body)
            .map(|t| t.clean_up())
    }
}

impl UrlTitles {
    /// If a file is larger than `max_kib` KiB the download is stopped
    pub fn new(max_kib: usize) -> Self {
        UrlTitles { max_kib: max_kib }
    }

    fn grep_url<'a>(&self, msg: &'a str) -> Option<Url<'a>> {
        let captures = URL_RE.captures(msg)?;
        debug!("Url captures: {:?}", captures);

        Some(captures.get(2)?.as_str().into())
    }

    fn url(&self, text: &str) -> Result<String, UrlError> {
        let url = self.grep_url(text)
            .ok_or(ErrorKind::MissingUrl)?
            .max_kib(self.max_kib);
        let body = url.request().context(ErrorKind::Download)?;

        let title = Title::find_clean_title(&body, url.as_str());
        let og_title = Title::find_clean_ogtitle(&body, url.as_str());

        let title = match (title, og_title) {
            (Ok(title), Ok(og_title)) => {
                if title.usefulness(url.as_str()) > og_title.usefulness(url.as_str()) {
                    title
                } else {
                    og_title
                }
            },
            (Ok(title), _) => title,
            (_, Ok(title)) => title,
            (Err(e), _) => Err(e)?,
        };

        Ok(title.into())
    }
}

impl Plugin for UrlTitles {
    fn execute(&self, _: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::PRIVMSG(_, ref msg) => if URL_RE.is_match(msg) {
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
