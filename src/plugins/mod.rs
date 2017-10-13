//! Collection of plugins included
mod help;
mod url;
mod emoji;
mod currency;
mod factoids;
mod keepnick;

pub use self::help::Help;
pub use self::url::Url;
pub use self::emoji::Emoji;
pub use self::currency::Currency;
pub use self::factoids::Factoids;
pub use self::keepnick::KeepNick;
