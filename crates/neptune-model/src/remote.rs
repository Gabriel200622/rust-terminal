use crate::Error;

/// Where a remote workspace's terminals run: an SSH destination exactly as the
/// system client accepts it (`host`, `user@host`, a configured alias or
/// `ssh://user@host:port`). Credentials are never part of it.
///
/// The destination becomes one argument of the client's command line, so a
/// value that could be read as an option or split into several is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote(String);

impl Remote {
    pub const MAX_LENGTH: usize = 255;

    pub fn parse(text: &str) -> Result<Self, Error> {
        let destination = text.trim();
        if destination.is_empty()
            || destination.len() > Self::MAX_LENGTH
            || destination.starts_with('-')
            || destination
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(Error::InvalidRemote);
        }
        Ok(Self(destination.to_owned()))
    }
    pub fn destination(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_the_client_accepts_are_kept_as_typed() {
        for text in [
            "devbox",
            "me@devbox.example",
            "ssh://me@devbox.example:2222",
            "me@2001:db8::1",
            "me@[2001:db8::1]",
        ] {
            assert_eq!(Remote::parse(text).unwrap().destination(), text);
        }
        assert_eq!(
            Remote::parse("  me@devbox \n").unwrap().destination(),
            "me@devbox"
        );
    }

    #[test]
    fn a_destination_cannot_become_an_option_or_several_arguments() {
        for text in [
            "",
            "   ",
            "-oProxyCommand=id",
            " -l root",
            "host -p 22",
            "host\tname",
            "host\nname",
            "host\u{0}",
            "host\u{1b}[31m",
        ] {
            assert_eq!(Remote::parse(text), Err(Error::InvalidRemote), "{text:?}");
        }
        assert!(Remote::parse(&"h".repeat(Remote::MAX_LENGTH)).is_ok());
        assert!(Remote::parse(&"h".repeat(Remote::MAX_LENGTH + 1)).is_err());
    }
}
