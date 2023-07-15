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

use crate::core::{ResponseCode, StatusResponse};

use super::{list::ListItem, ImapResponse, Sequence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    pub tag: String,
    pub mailbox_name: String,
    pub condstore: bool,
    pub qresync: Option<QResync>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QResync {
    pub uid_validity: u32,
    pub modseq: u64,
    pub known_uids: Option<Sequence>,
    pub seq_match: Option<(Sequence, Sequence)>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub mailbox: ListItem,
    pub total_messages: usize,
    pub recent_messages: usize,
    pub unseen_seq: u32,
    pub uid_validity: u32,
    pub uid_next: u32,
    pub is_rev2: bool,
    pub closed_previous: bool,
    pub highest_modseq: Option<u32>,
    pub mailbox_id: String,
}

#[derive(Debug, Clone)]
pub struct Exists {
    pub total_messages: usize,
}

impl ImapResponse for Response {
    fn serialize(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(100);
        if self.closed_previous {
            buf = StatusResponse::ok("Closed previous mailbox")
                .with_code(ResponseCode::Closed)
                .serialize(buf);
        }
        buf.extend_from_slice(b"* ");
        buf.extend_from_slice(self.total_messages.to_string().as_bytes());
        buf.extend_from_slice(
            b" EXISTS\r\n* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n",
        );
        if self.is_rev2 {
            self.mailbox.serialize(&mut buf, self.is_rev2, false);
        } else {
            buf.extend_from_slice(b"* ");
            buf.extend_from_slice(self.recent_messages.to_string().as_bytes());
            buf.extend_from_slice(b" RECENT\r\n");
            if self.unseen_seq > 0 {
                buf.extend_from_slice(b"* OK [UNSEEN ");
                buf.extend_from_slice(self.unseen_seq.to_string().as_bytes());
                buf.extend_from_slice(b"] Unseen messages\r\n");
            }
        }
        buf.extend_from_slice(
            b"* OK [PERMANENTFLAGS (\\Deleted \\Seen \\Answered \\Flagged \\Draft \\*)] All allowed\r\n",
        );
        buf.extend_from_slice(b"* OK [UIDVALIDITY ");
        buf.extend_from_slice(self.uid_validity.to_string().as_bytes());
        buf.extend_from_slice(b"] UIDs valid\r\n* OK [UIDNEXT ");
        buf.extend_from_slice(self.uid_next.to_string().as_bytes());
        buf.extend_from_slice(b"] Next predicted UID\r\n");
        if let Some(highest_modseq) = self.highest_modseq {
            buf.extend_from_slice(b"* OK [HIGHESTMODSEQ ");
            buf.extend_from_slice(highest_modseq.to_string().as_bytes());
            buf.extend_from_slice(b"] Highest Modseq\r\n");
        }
        buf.extend_from_slice(b"* OK [MAILBOXID (");
        buf.extend_from_slice(self.mailbox_id.as_bytes());
        buf.extend_from_slice(b")] Unique Mailbox ID\r\n");
        buf
    }
}

impl Exists {
    pub fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"* ");
        buf.extend_from_slice(self.total_messages.to_string().as_bytes());
        buf.extend_from_slice(b" EXISTS\r\n");
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(15);
        self.serialize(&mut buf);
        buf
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::{list::ListItem, ImapResponse};

    #[test]
    fn serialize_select() {
        for (mut response, _tag, expected_v2, expected_v1) in [
            (
                super::Response {
                    mailbox: ListItem::new("INBOX"),
                    total_messages: 172,
                    recent_messages: 5,
                    unseen_seq: 3,
                    uid_validity: 3857529045,
                    uid_next: 4392,
                    closed_previous: false,
                    is_rev2: true,
                    highest_modseq: 100.into(),
                    mailbox_id: "abc".into(),
                },
                "A142",
                concat!(
                    "* 172 EXISTS\r\n",
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n",
                    "* LIST () \"/\" \"INBOX\"\r\n",
                    "* OK [PERMANENTFLAGS (\\Deleted \\Seen \\Answered \\Flagged \\Draft \\*)] All allowed\r\n",
                    "* OK [UIDVALIDITY 3857529045] UIDs valid\r\n",
                    "* OK [UIDNEXT 4392] Next predicted UID\r\n",
                    "* OK [HIGHESTMODSEQ 100] Highest Modseq\r\n",
                    "* OK [MAILBOXID (abc)] Unique Mailbox ID\r\n"
                ),
                concat!(
                    "* 172 EXISTS\r\n",
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n",
                    "* 5 RECENT\r\n",
                    "* OK [UNSEEN 3] Unseen messages\r\n",
                    "* OK [PERMANENTFLAGS (\\Deleted \\Seen \\Answered \\Flagged \\Draft \\*)] All allowed\r\n",
                    "* OK [UIDVALIDITY 3857529045] UIDs valid\r\n",
                    "* OK [UIDNEXT 4392] Next predicted UID\r\n",
                    "* OK [HIGHESTMODSEQ 100] Highest Modseq\r\n",
                    "* OK [MAILBOXID (abc)] Unique Mailbox ID\r\n"
                ),
            ),
            (
                super::Response {
                    mailbox: ListItem::new("~peter/mail/台北/日本語"),
                    total_messages: 172,
                    recent_messages: 5,
                    unseen_seq: 3,
                    uid_validity: 3857529045,
                    uid_next: 4392,
                    closed_previous: true,
                    is_rev2: true,
                    highest_modseq: None,
                    mailbox_id: "abc".into(),
                },
                "A142",
                concat!(
                    "* OK [CLOSED] Closed previous mailbox\r\n",
                    "* 172 EXISTS\r\n",
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n",
                    "* LIST () \"/\" \"~peter/mail/台北/日本語\" (\"OLDNAME\" ",
                    "(\"~peter/mail/&U,BTFw-/&ZeVnLIqe-\"))\r\n",
                    "* OK [PERMANENTFLAGS (\\Deleted \\Seen \\Answered \\Flagged \\Draft \\*)] All allowed\r\n",
                    "* OK [UIDVALIDITY 3857529045] UIDs valid\r\n",
                    "* OK [UIDNEXT 4392] Next predicted UID\r\n",
                    "* OK [MAILBOXID (abc)] Unique Mailbox ID\r\n"
                ),
                concat!(
                    "* OK [CLOSED] Closed previous mailbox\r\n",
                    "* 172 EXISTS\r\n",
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n",
                    "* 5 RECENT\r\n",
                    "* OK [UNSEEN 3] Unseen messages\r\n",
                    "* OK [PERMANENTFLAGS (\\Deleted \\Seen \\Answered \\Flagged \\Draft \\*)] All allowed\r\n",
                    "* OK [UIDVALIDITY 3857529045] UIDs valid\r\n",
                    "* OK [UIDNEXT 4392] Next predicted UID\r\n",
                    "* OK [MAILBOXID (abc)] Unique Mailbox ID\r\n"
                ),
            ),
        ] {
            let response_v2 = String::from_utf8(response.clone().serialize()).unwrap();
            response.is_rev2 = false;
            let response_v1 = String::from_utf8(response.serialize()).unwrap();

            assert_eq!(response_v2, expected_v2);
            assert_eq!(response_v1, expected_v1);
        }
    }
}
