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

use std::borrow::Cow;
use std::iter::Peekable;
use std::vec::IntoIter;

use jmap_client::core::query::Operator;
use mail_parser::decoders::charsets::map::get_charset_decoder;
use mail_parser::decoders::charsets::DecoderFnc;

use crate::core::receiver::{Request, Token};
use crate::core::{Command, Flag};
use crate::protocol::search::{self, Filter};
use crate::protocol::search::{ModSeqEntry, ResultOption};
use crate::protocol::ProtocolVersion;

use super::{parse_date, parse_number, parse_sequence_set};

impl Request<Command> {
    #[allow(clippy::while_let_on_iterator)]
    pub fn parse_search(self, version: ProtocolVersion) -> crate::core::Result<search::Arguments> {
        if self.tokens.is_empty() {
            return Err(self.into_error("Missing search criteria."));
        }

        let mut tokens = self.tokens.into_iter().peekable();
        let mut result_options = Vec::new();
        let mut decoder = None;
        let mut is_esearch = version.is_rev2();

        loop {
            match tokens.peek() {
                Some(Token::Argument(value)) if value.eq_ignore_ascii_case(b"return") => {
                    tokens.next();
                    is_esearch = true;
                    result_options =
                        parse_result_options(&mut tokens).map_err(|v| (self.tag.as_str(), v))?;
                }
                Some(Token::Argument(value)) if value.eq_ignore_ascii_case(b"charset") => {
                    tokens.next();
                    decoder = get_charset_decoder(
                        &tokens
                            .next()
                            .ok_or((self.tag.as_str(), "Missing charset."))?
                            .unwrap_bytes(),
                    );
                }
                _ => break,
            }
        }

        let mut filters =
            parse_filters(&mut tokens, decoder).map_err(|v| (self.tag.as_str(), v))?;

        match filters.len() {
            0 => Err((self.tag.as_str(), "No filters found in command.").into()),
            1 => Ok(search::Arguments {
                tag: self.tag,
                result_options,
                filter: filters.pop().unwrap(),
                sort: None,
                is_esearch,
            }),
            _ => Ok(search::Arguments {
                tag: self.tag,
                result_options,
                filter: Filter::Operator(Operator::And, filters),
                sort: None,
                is_esearch,
            }),
        }
    }
}

pub fn parse_result_options(
    tokens: &mut Peekable<IntoIter<Token>>,
) -> super::Result<Vec<ResultOption>> {
    let mut result_options = Vec::new();
    if tokens
        .next()
        .map_or(true, |token| !token.is_parenthesis_open())
    {
        return Err(Cow::from("Invalid result option, expected parenthesis."));
    }

    for token in tokens {
        match token {
            Token::ParenthesisClose => break,
            Token::Argument(value) => {
                result_options.push(ResultOption::parse(&value)?);
            }
            _ => return Err(Cow::from("Invalid result option argument.")),
        }
    }

    Ok(result_options)
}

pub fn parse_filters(
    tokens: &mut Peekable<IntoIter<Token>>,
    decoder: Option<DecoderFnc>,
) -> super::Result<Vec<Filter>> {
    let mut filters = Vec::new();
    let mut operator = Operator::And;
    let mut filters_stack = Vec::new();

    while let Some(token) = tokens.next() {
        let mut found_parenthesis = false;
        match token {
            Token::Argument(value) => {
                if value.eq_ignore_ascii_case(b"ALL") {
                    filters.push(Filter::All);
                } else if value.eq_ignore_ascii_case(b"ANSWERED") {
                    filters.push(Filter::Answered);
                } else if value.eq_ignore_ascii_case(b"BCC") {
                    filters.push(Filter::Bcc(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"BEFORE") {
                    filters.push(Filter::All);
                } else if value.eq_ignore_ascii_case(b"BODY") {
                    filters.push(Filter::Body(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"CC") {
                    filters.push(Filter::Cc(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"DELETED") {
                    filters.push(Filter::Deleted);
                } else if value.eq_ignore_ascii_case(b"DRAFT") {
                    filters.push(Filter::Draft);
                } else if value.eq_ignore_ascii_case(b"FLAGGED") {
                    filters.push(Filter::Flagged);
                } else if value.eq_ignore_ascii_case(b"FROM") {
                    filters.push(Filter::From(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"HEADER") {
                    filters.push(Filter::Header(
                        decode_argument(tokens, decoder)?,
                        decode_argument(tokens, decoder)?,
                    ));
                } else if value.eq_ignore_ascii_case(b"KEYWORD") {
                    filters.push(Filter::Keyword(Flag::parse_imap(
                        tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected keyword"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"LARGER") {
                    filters.push(Filter::Larger(parse_number::<u32>(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected integer"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"ON") {
                    filters.push(Filter::On(parse_date(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected date"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SEEN") {
                    filters.push(Filter::Seen);
                } else if value.eq_ignore_ascii_case(b"SENTBEFORE") {
                    filters.push(Filter::SentBefore(parse_date(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected date"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SENTON") {
                    filters.push(Filter::SentOn(parse_date(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected date"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SENTSINCE") {
                    filters.push(Filter::SentSince(parse_date(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected date"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SINCE") {
                    filters.push(Filter::Since(parse_date(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected date"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SMALLER") {
                    filters.push(Filter::Smaller(parse_number::<u32>(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected integer"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"SUBJECT") {
                    filters.push(Filter::Subject(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"TEXT") {
                    filters.push(Filter::Text(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"TO") {
                    filters.push(Filter::To(decode_argument(tokens, decoder)?));
                } else if value.eq_ignore_ascii_case(b"UID") {
                    filters.push(Filter::Sequence(
                        parse_sequence_set(
                            &tokens
                                .next()
                                .ok_or_else(|| Cow::from("Missing sequence set."))?
                                .unwrap_bytes(),
                        )?,
                        true,
                    ));
                } else if value.eq_ignore_ascii_case(b"UNANSWERED") {
                    filters.push(Filter::Unanswered);
                } else if value.eq_ignore_ascii_case(b"UNDELETED") {
                    filters.push(Filter::Undeleted);
                } else if value.eq_ignore_ascii_case(b"UNDRAFT") {
                    filters.push(Filter::Undraft);
                } else if value.eq_ignore_ascii_case(b"UNFLAGGED") {
                    filters.push(Filter::Unflagged);
                } else if value.eq_ignore_ascii_case(b"UNKEYWORD") {
                    filters.push(Filter::Unkeyword(Flag::parse_imap(
                        tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected keyword"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"UNSEEN") {
                    filters.push(Filter::Unseen);
                } else if value.eq_ignore_ascii_case(b"OLDER") {
                    filters.push(Filter::Older(parse_number::<u32>(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected integer"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"YOUNGER") {
                    filters.push(Filter::Younger(parse_number::<u32>(
                        &tokens
                            .next()
                            .ok_or_else(|| Cow::from("Expected integer"))?
                            .unwrap_bytes(),
                    )?));
                } else if value.eq_ignore_ascii_case(b"OLD") {
                    filters.push(Filter::Old);
                } else if value.eq_ignore_ascii_case(b"NEW") {
                    filters.push(Filter::New);
                } else if value.eq_ignore_ascii_case(b"RECENT") {
                    filters.push(Filter::Recent);
                } else if value.eq_ignore_ascii_case(b"MODSEQ") {
                    let param = tokens
                        .next()
                        .ok_or_else(|| Cow::from("Missing MODSEQ parameters."))?
                        .unwrap_bytes();
                    if param.is_empty() || param.iter().any(|ch| !ch.is_ascii_digit()) {
                        if param.len() <= 7 || !param.starts_with(b"/flags/") {
                            return Err(format!(
                                "Unsupported MODSEQ parameter '{}'.",
                                String::from_utf8_lossy(&param)
                            )
                            .into());
                        }
                        let flag = Flag::parse_imap((param[7..]).to_vec())?;
                        let mod_seq_entry = match tokens.next() {
                            Some(Token::Argument(value)) if value.eq_ignore_ascii_case(b"all") => {
                                ModSeqEntry::All(flag)
                            }
                            Some(Token::Argument(value))
                                if value.eq_ignore_ascii_case(b"shared") =>
                            {
                                ModSeqEntry::Shared(flag)
                            }
                            Some(Token::Argument(value)) if value.eq_ignore_ascii_case(b"priv") => {
                                ModSeqEntry::Private(flag)
                            }
                            Some(token) => {
                                return Err(
                                    format!("Unsupported MODSEQ parameter '{}'.", token).into()
                                );
                            }
                            None => {
                                return Err("Missing MODSEQ entry-type-req parameter.".into());
                            }
                        };
                        filters.push(Filter::ModSeq((
                            parse_number::<u64>(
                                &tokens
                                    .next()
                                    .ok_or_else(|| {
                                        Cow::from("Missing MODSEQ mod-sequence-valzer parameter.")
                                    })?
                                    .unwrap_bytes(),
                            )?,
                            mod_seq_entry,
                        )));
                    } else {
                        filters.push(Filter::ModSeq((
                            parse_number::<u64>(&param)?,
                            ModSeqEntry::None,
                        )));
                    }
                } else if value.eq_ignore_ascii_case(b"EMAILID") {
                    let argument = tokens
                        .next()
                        .ok_or_else(|| Cow::from("Expected an EMAILID value."))?
                        .unwrap_string()?;
                    if let Some((_, email_id)) = argument.split_once('-') {
                        filters.push(Filter::EmailId(email_id.to_string()));
                    } else {
                        return Err(Cow::from("Malformed EMAILID value."));
                    }
                } else if value.eq_ignore_ascii_case(b"THREADID") {
                    let argument = tokens
                        .next()
                        .ok_or_else(|| Cow::from("Expected an THREADID value."))?
                        .unwrap_string()?;
                    if let Some((_, thread_id)) = argument.split_once('-') {
                        filters.push(Filter::ThreadId(thread_id.to_string()));
                    } else {
                        return Err(Cow::from("Malformed THREADID value."));
                    }
                } else if value.eq_ignore_ascii_case(b"OR") {
                    if filters_stack.len() > 10 {
                        return Err(Cow::from("Too many nested filters"));
                    }

                    filters_stack.push((filters, operator));
                    filters = Vec::with_capacity(2);
                    operator = Operator::Or;
                    continue;
                } else if value.eq_ignore_ascii_case(b"NOT") {
                    if filters_stack.len() > 10 {
                        return Err(Cow::from("Too many nested filters"));
                    }

                    filters_stack.push((filters, operator));
                    filters = Vec::with_capacity(1);
                    operator = Operator::Not;
                    continue;
                } else {
                    filters.push(Filter::Sequence(parse_sequence_set(&value)?, false));
                }
            }
            Token::ParenthesisOpen => {
                if filters_stack.len() > 10 {
                    return Err(Cow::from("Too many nested filters"));
                }

                filters_stack.push((filters, operator));
                filters = Vec::with_capacity(5);
                operator = Operator::And;
                continue;
            }
            Token::ParenthesisClose => {
                if filters_stack.is_empty() {
                    return Err(Cow::from("Unexpected parenthesis."));
                }

                found_parenthesis = true;
            }
            token => return Err(format!("Unexpected token {:?}.", token.to_string()).into()),
        }

        if !filters_stack.is_empty()
            && (found_parenthesis
                || (operator == Operator::Or && filters.len() == 2)
                || (operator == Operator::Not && filters.len() == 1))
        {
            while let Some((mut prev_filters, prev_operator)) = filters_stack.pop() {
                if operator == Operator::And
                    && (prev_operator != Operator::Or || filters.len() == 1)
                {
                    prev_filters.extend(filters);
                } else {
                    prev_filters.push(Filter::Operator(operator, filters));
                }
                operator = prev_operator;
                filters = prev_filters;

                if operator == Operator::And || (operator == Operator::Or && filters.len() < 2) {
                    break;
                }
            }
        }
    }
    Ok(filters)
}

pub fn decode_argument(
    tokens: &mut Peekable<IntoIter<Token>>,
    decoder: Option<DecoderFnc>,
) -> super::Result<String> {
    let argument = tokens
        .next()
        .ok_or_else(|| Cow::from("Expected string."))?
        .unwrap_bytes();

    if let Some(decoder) = decoder {
        Ok(decoder(&argument))
    } else {
        Ok(String::from_utf8(argument.to_vec())
            .map_err(|_| Cow::from("Invalid UTF-8 argument."))?)
    }
}

impl ResultOption {
    pub fn parse(value: &[u8]) -> super::Result<Self> {
        if value.eq_ignore_ascii_case(b"min") {
            Ok(Self::Min)
        } else if value.eq_ignore_ascii_case(b"max") {
            Ok(Self::Max)
        } else if value.eq_ignore_ascii_case(b"all") {
            Ok(Self::All)
        } else if value.eq_ignore_ascii_case(b"count") {
            Ok(Self::Count)
        } else if value.eq_ignore_ascii_case(b"save") {
            Ok(Self::Save)
        } else if value.eq_ignore_ascii_case(b"context") {
            Ok(Self::Context)
        } else {
            Err(format!("Invalid result option {:?}", String::from_utf8_lossy(value)).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::{receiver::Receiver, Flag},
        protocol::{
            search::{self, Filter, ModSeqEntry, ResultOption},
            ProtocolVersion, Sequence,
        },
    };

    #[test]
    fn parse_search() {
        let mut receiver = Receiver::new();

        for (command, arguments) in [
            (
                b"A282 SEARCH RETURN (MIN COUNT) FLAGGED SINCE 1-Feb-1994 NOT FROM \"Smith\"\r\n"
                    .to_vec(),
                search::Arguments {
                    tag: "A282".to_string(),
                    result_options: vec![ResultOption::Min, ResultOption::Count],
                    filter: Filter::and([
                        Filter::Flagged,
                        Filter::Since(760060800),
                        Filter::not([Filter::From("Smith".to_string())]),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"A283 SEARCH RETURN () FLAGGED SINCE 1-Feb-1994 NOT FROM \"Smith\"\r\n".to_vec(),
                search::Arguments {
                    tag: "A283".to_string(),
                    result_options: vec![],
                    filter: Filter::and([
                        Filter::Flagged,
                        Filter::Since(760060800),
                        Filter::not([Filter::From("Smith".to_string())]),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"A301 SEARCH $ SMALLER 4096\r\n".to_vec(),
                search::Arguments {
                    tag: "A301".to_string(),
                    result_options: vec![],
                    filter: Filter::and([Filter::seq_saved_search(), Filter::Smaller(4096)]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                "P283 SEARCH CHARSET UTF-8 (OR $ 1,3000:3021) TEXT {8+}\r\nмать\r\n"
                    .as_bytes()
                    .to_vec(),
                search::Arguments {
                    tag: "P283".to_string(),
                    result_options: vec![],
                    filter: Filter::and([
                        Filter::or([
                            Filter::seq_saved_search(),
                            Filter::Sequence(
                                Sequence::List {
                                    items: vec![
                                        Sequence::number(1),
                                        Sequence::range(3000.into(), 3021.into()),
                                    ],
                                },
                                false,
                            ),
                        ]),
                        Filter::Text("мать".to_string()),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"F282 SEARCH RETURN (SAVE) KEYWORD $Junk\r\n".to_vec(),
                search::Arguments {
                    tag: "F282".to_string(),
                    result_options: vec![ResultOption::Save],
                    filter: Filter::Keyword(Flag::Junk),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"F282 SEARCH OR OR FROM hello@world.com TO ".to_vec(),
                    b"test@example.com OR BCC jane@foobar.com ".to_vec(),
                    b"CC john@doe.com\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "F282".to_string(),
                    result_options: vec![],
                    filter: Filter::or([
                        Filter::or([
                            Filter::From("hello@world.com".to_string()),
                            Filter::To("test@example.com".to_string()),
                        ]),
                        Filter::or([
                            Filter::Bcc("jane@foobar.com".to_string()),
                            Filter::Cc("john@doe.com".to_string()),
                        ]),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"abc SEARCH OR SMALLER 10000 OR ".to_vec(),
                    b"HEADER Subject \"ravioli festival\" ".to_vec(),
                    b"HEADER From \"dr. ravioli\"\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "abc".to_string(),
                    result_options: vec![],
                    filter: Filter::or([
                        Filter::Smaller(10000),
                        Filter::or([
                            Filter::Header("Subject".to_string(), "ravioli festival".to_string()),
                            Filter::Header("From".to_string(), "dr. ravioli".to_string()),
                        ]),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"abc SEARCH (DELETED SEEN ANSWERED) ".to_vec(),
                    b"NOT (FROM john TO jane BCC bill) ".to_vec(),
                    b"(1,30:* UID 1,2,3,4 $)\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "abc".to_string(),
                    result_options: vec![],
                    filter: Filter::and([
                        Filter::Deleted,
                        Filter::Seen,
                        Filter::Answered,
                        Filter::not([
                            Filter::From("john".to_string()),
                            Filter::To("jane".to_string()),
                            Filter::Bcc("bill".to_string()),
                        ]),
                        Filter::Sequence(
                            Sequence::List {
                                items: vec![Sequence::number(1), Sequence::range(30.into(), None)],
                            },
                            false,
                        ),
                        Filter::Sequence(
                            Sequence::List {
                                items: vec![
                                    Sequence::number(1),
                                    Sequence::number(2),
                                    Sequence::number(3),
                                    Sequence::number(4),
                                ],
                            },
                            true,
                        ),
                        Filter::seq_saved_search(),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"abc SEARCH *:* UID *:100,100:* ".to_vec(),
                    b"(FLAGGED (DRAFT (DELETED (ANSWERED)))) ".to_vec(),
                    b"OR (SENTON 20-Nov-2022) (LARGER 8196)\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "abc".to_string(),
                    result_options: vec![],
                    filter: Filter::and([
                        Filter::seq_range(None, None),
                        Filter::Sequence(
                            Sequence::List {
                                items: vec![
                                    Sequence::range(None, 100.into()),
                                    Sequence::range(100.into(), None),
                                ],
                            },
                            true,
                        ),
                        Filter::Flagged,
                        Filter::Draft,
                        Filter::Deleted,
                        Filter::Answered,
                        Filter::or([Filter::SentOn(1668902400), Filter::Larger(8196)]),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"abc SEARCH NOT (FROM john OR TO jane CC bill) ".to_vec(),
                    b"OR (UNDELETED ALL) ($ NOT FLAGGED) ".to_vec(),
                    b"(((KEYWORD \"tps report\")))\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "abc".to_string(),
                    result_options: vec![],
                    filter: Filter::and([
                        Filter::not([
                            Filter::From("john".to_string()),
                            Filter::or([
                                Filter::To("jane".to_string()),
                                Filter::Cc("bill".to_string()),
                            ]),
                        ]),
                        Filter::or([
                            Filter::and([Filter::Undeleted, Filter::All]),
                            Filter::and([
                                Filter::seq_saved_search(),
                                Filter::not([Filter::Flagged]),
                            ]),
                        ]),
                        Filter::Keyword(Flag::Keyword("tps report".to_string())),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                [
                    b"B283 SEARCH RETURN (SAVE MIN MAX) CHARSET KOI8-R TEXT ".to_vec(),
                    b"{11+}\r\n\xf0\xd2\xc9\xd7\xc5\xd4, \xcd\xc9\xd2\r\n".to_vec(),
                ]
                .concat(),
                search::Arguments {
                    tag: "B283".to_string(),
                    result_options: vec![ResultOption::Save, ResultOption::Min, ResultOption::Max],
                    filter: Filter::Text("Привет, мир".to_string()),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"B283 SEARCH CHARSET BIG5 FROM \"\xa7A\xa6n\xa1A\xa5@\xac\xc9\"\r\n".to_vec(),
                search::Arguments {
                    tag: "B283".to_string(),
                    result_options: vec![],
                    filter: Filter::From("你好，世界".to_string()),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"a SEARCH MODSEQ \"/flags/\\draft\" all 620162338\r\n".to_vec(),
                search::Arguments {
                    tag: "a".to_string(),
                    result_options: vec![],
                    filter: Filter::ModSeq((620162338, ModSeqEntry::All(Flag::Draft))),
                    is_esearch: true,
                    sort: None,
                },
            ),
            (
                b"t SEARCH OR NOT MODSEQ 720162338 LARGER 50000\r\n".to_vec(),
                search::Arguments {
                    tag: "t".to_string(),
                    result_options: vec![],
                    filter: Filter::or(vec![
                        Filter::not(vec![Filter::ModSeq((720162338, ModSeqEntry::None))]),
                        Filter::Larger(50000),
                    ]),
                    is_esearch: true,
                    sort: None,
                },
            ),
        ] {
            let command_str = String::from_utf8_lossy(&command).into_owned();
            assert_eq!(
                receiver
                    .parse(&mut command.iter())
                    .unwrap()
                    .parse_search(ProtocolVersion::Rev2)
                    .expect(&command_str),
                arguments,
                "{}",
                command_str
            );
        }
    }
}
