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

use jmap_client::core::query::Operator;
use mail_parser::decoders::charsets::map::get_charset_decoder;

use crate::{
    core::{
        receiver::{Request, Token},
        Command,
    },
    protocol::search::{Arguments, Comparator, Filter, Sort},
};

use super::search::{parse_filters, parse_result_options};

impl Request<Command> {
    #[allow(clippy::while_let_on_iterator)]
    pub fn parse_sort(self) -> crate::core::Result<Arguments> {
        if self.tokens.is_empty() {
            return Err(self.into_error("Missing sort criteria."));
        }

        let mut tokens = self.tokens.into_iter().peekable();
        let mut sort = Vec::new();

        let (result_options, is_esearch) = match tokens.peek() {
            Some(Token::Argument(value)) if value.eq_ignore_ascii_case(b"return") => {
                tokens.next();
                (
                    parse_result_options(&mut tokens).map_err(|v| (self.tag.as_str(), v))?,
                    true,
                )
            }
            _ => (Vec::new(), false),
        };

        if tokens
            .next()
            .map_or(true, |token| !token.is_parenthesis_open())
        {
            return Err((
                self.tag.as_str(),
                "Expected sort criteria between parentheses.",
            )
                .into());
        }

        let mut is_ascending = true;
        while let Some(token) = tokens.next() {
            match token {
                Token::ParenthesisClose => break,
                Token::Argument(value) => {
                    if value.eq_ignore_ascii_case(b"REVERSE") {
                        is_ascending = false;
                    } else {
                        sort.push(Comparator {
                            sort: Sort::parse(&value).map_err(|v| (self.tag.as_str(), v))?,
                            ascending: is_ascending,
                        });
                        is_ascending = true;
                    }
                }
                _ => return Err((self.tag.as_str(), "Invalid result option argument.").into()),
            }
        }

        if sort.is_empty() {
            return Err((self.tag.as_str(), "Missing sort criteria.").into());
        }

        let decoder = get_charset_decoder(
            &tokens
                .next()
                .ok_or((self.tag.as_str(), "Missing charset."))?
                .unwrap_bytes(),
        );

        let mut filters =
            parse_filters(&mut tokens, decoder).map_err(|v| (self.tag.as_str(), v))?;
        match filters.len() {
            0 => Err((self.tag.as_str(), "No filters found in command.").into()),
            1 => Ok(Arguments {
                sort: sort.into(),
                result_options,
                filter: filters.pop().unwrap(),
                is_esearch,
                tag: self.tag,
            }),
            _ => Ok(Arguments {
                sort: sort.into(),
                result_options,
                filter: Filter::Operator(Operator::And, filters),
                is_esearch,
                tag: self.tag,
            }),
        }
    }
}

impl Sort {
    pub fn parse(value: &[u8]) -> super::Result<Self> {
        if value.eq_ignore_ascii_case(b"ARRIVAL") {
            Ok(Self::Arrival)
        } else if value.eq_ignore_ascii_case(b"CC") {
            Ok(Self::Cc)
        } else if value.eq_ignore_ascii_case(b"DATE") {
            Ok(Self::Date)
        } else if value.eq_ignore_ascii_case(b"FROM") {
            Ok(Self::From)
        } else if value.eq_ignore_ascii_case(b"SIZE") {
            Ok(Self::Size)
        } else if value.eq_ignore_ascii_case(b"SUBJECT") {
            Ok(Self::Subject)
        } else if value.eq_ignore_ascii_case(b"TO") {
            Ok(Self::To)
        } else if value.eq_ignore_ascii_case(b"DISPLAYFROM") {
            Ok(Self::DisplayFrom)
        } else if value.eq_ignore_ascii_case(b"DISPLAYTO") {
            Ok(Self::DisplayTo)
        } else {
            Err(format!("Invalid sort criteria {:?}", String::from_utf8_lossy(value)).into())
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        core::{receiver::Receiver, Flag},
        protocol::search::{Arguments, Comparator, Filter, ResultOption, Sort},
    };

    #[test]
    fn parse_sort() {
        let mut receiver = Receiver::new();

        for (command, arguments) in [
            (
                b"A282 SORT (SUBJECT) UTF-8 SINCE 1-Feb-1994\r\n".to_vec(),
                Arguments {
                    sort: vec![Comparator {
                        sort: Sort::Subject,
                        ascending: true,
                    }]
                    .into(),
                    filter: Filter::Since(760060800),
                    result_options: Vec::new(),
                    is_esearch: false,
                    tag: "A282".to_string(),
                },
            ),
            (
                b"A283 SORT (SUBJECT REVERSE DATE) UTF-8 ALL\r\n".to_vec(),
                Arguments {
                    sort: vec![
                        Comparator {
                            sort: Sort::Subject,
                            ascending: true,
                        },
                        Comparator {
                            sort: Sort::Date,
                            ascending: false,
                        },
                    ]
                    .into(),
                    filter: Filter::All,
                    result_options: Vec::new(),
                    is_esearch: false,
                    tag: "A283".to_string(),
                },
            ),
            (
                b"A284 SORT (SUBJECT) US-ASCII TEXT \"not in mailbox\"\r\n".to_vec(),
                Arguments {
                    sort: vec![Comparator {
                        sort: Sort::Subject,
                        ascending: true,
                    }]
                    .into(),
                    filter: Filter::Text("not in mailbox".to_string()),
                    result_options: Vec::new(),
                    is_esearch: false,
                    tag: "A284".to_string(),
                },
            ),
            (
                [
                    b"A284 SORT (REVERSE ARRIVAL FROM) iso-8859-6 SUBJECT ".to_vec(),
                    b"\"\xe5\xd1\xcd\xc8\xc7 \xc8\xc7\xe4\xd9\xc7\xe4\xe5\"\r\n".to_vec(),
                ]
                .concat(),
                Arguments {
                    sort: vec![
                        Comparator {
                            sort: Sort::Arrival,
                            ascending: false,
                        },
                        Comparator {
                            sort: Sort::From,
                            ascending: true,
                        },
                    ]
                    .into(),
                    filter: Filter::Subject("مرحبا بالعالم".to_string()),
                    result_options: Vec::new(),
                    is_esearch: false,
                    tag: "A284".to_string(),
                },
            ),
            (
                [
                    b"E01 UID SORT RETURN (COUNT) (REVERSE DATE) ".to_vec(),
                    b"UTF-8 UNDELETED UNKEYWORD $Junk\r\n".to_vec(),
                ]
                .concat(),
                Arguments {
                    sort: vec![Comparator {
                        sort: Sort::Date,
                        ascending: false,
                    }]
                    .into(),
                    filter: Filter::and(vec![Filter::Undeleted, Filter::Unkeyword(Flag::Junk)]),
                    result_options: vec![ResultOption::Count],
                    is_esearch: true,
                    tag: "E01".to_string(),
                },
            ),
        ] {
            let command_str = String::from_utf8_lossy(&command).into_owned();

            assert_eq!(
                receiver
                    .parse(&mut command.iter())
                    .unwrap()
                    .parse_sort()
                    .expect(&command_str),
                arguments,
                "{}",
                command_str
            );
        }
    }
}
