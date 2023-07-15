/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart IMAP Server.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use crate::core::receiver::{Request, Token};
use crate::core::utf7::utf7_maybe_decode;
use crate::core::Command;
use crate::protocol::status::Status;
use crate::protocol::{status, ProtocolVersion};

impl Request<Command> {
    pub fn parse_status(self, version: ProtocolVersion) -> crate::core::Result<status::Arguments> {
        match self.tokens.len() {
            0..=3 => Err(self.into_error("Missing arguments.")),
            len => {
                let mut tokens = self.tokens.into_iter();
                let mailbox_name = utf7_maybe_decode(
                    tokens
                        .next()
                        .unwrap()
                        .unwrap_string()
                        .map_err(|v| (self.tag.as_ref(), v))?,
                    version,
                );
                let mut items = Vec::with_capacity(len - 2);

                if tokens
                    .next()
                    .map_or(true, |token| !token.is_parenthesis_open())
                {
                    return Err((
                        self.tag.as_str(),
                        "Expected parenthesis after mailbox name.",
                    )
                        .into());
                }

                #[allow(clippy::while_let_on_iterator)]
                while let Some(token) = tokens.next() {
                    match token {
                        Token::ParenthesisClose => break,
                        Token::Argument(value) => {
                            items.push(Status::parse(&value).map_err(|v| (self.tag.as_str(), v))?);
                        }
                        _ => {
                            return Err((
                                self.tag.as_str(),
                                "Invalid status return option argument.",
                            )
                                .into())
                        }
                    }
                }

                if !items.is_empty() {
                    Ok(status::Arguments {
                        tag: self.tag,
                        mailbox_name,
                        items,
                    })
                } else {
                    Err((self.tag, "At least one status item is required.").into())
                }
            }
        }
    }
}

impl Status {
    pub fn parse(value: &[u8]) -> super::Result<Self> {
        if value.eq_ignore_ascii_case(b"messages") {
            Ok(Self::Messages)
        } else if value.eq_ignore_ascii_case(b"uidnext") {
            Ok(Self::UidNext)
        } else if value.eq_ignore_ascii_case(b"uidvalidity") {
            Ok(Self::UidValidity)
        } else if value.eq_ignore_ascii_case(b"unseen") {
            Ok(Self::Unseen)
        } else if value.eq_ignore_ascii_case(b"deleted") {
            Ok(Self::Deleted)
        } else if value.eq_ignore_ascii_case(b"size") {
            Ok(Self::Size)
        } else if value.eq_ignore_ascii_case(b"highestmodseq") {
            Ok(Self::HighestModSeq)
        } else if value.eq_ignore_ascii_case(b"mailboxid") {
            Ok(Self::MailboxId)
        } else if value.eq_ignore_ascii_case(b"recent") {
            Ok(Self::Recent)
        } else {
            Err(format!(
                "Invalid status option '{}'.",
                String::from_utf8_lossy(value)
            )
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::receiver::Receiver,
        protocol::{status, ProtocolVersion},
    };

    #[test]
    fn parse_status() {
        let mut receiver = Receiver::new();

        assert_eq!(
            receiver
                .parse(
                    &mut "A042 STATUS blurdybloop (UIDNEXT MESSAGES)\r\n"
                        .as_bytes()
                        .iter()
                )
                .unwrap()
                .parse_status(ProtocolVersion::Rev2)
                .unwrap(),
            status::Arguments {
                tag: "A042".to_string(),
                mailbox_name: "blurdybloop".to_string(),
                items: vec![status::Status::UidNext, status::Status::Messages],
            }
        );
    }
}
