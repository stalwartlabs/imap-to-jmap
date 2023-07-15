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

/*

   l - lookup (mailbox is visible to LIST/LSUB commands, SUBSCRIBE
       mailbox)
   r - read (SELECT the mailbox, perform STATUS)
   s - keep seen/unseen information across sessions (set or clear
       \SEEN flag via STORE, also set \SEEN during APPEND/COPY/
       FETCH BODY[...])
   w - write (set or clear flags other than \SEEN and \DELETED via
       STORE, also set them during APPEND/COPY)
   i - insert (perform APPEND, COPY into mailbox)
   p - post (send mail to submission address for mailbox,
       not enforced by IMAP4 itself)
   k - create mailboxes (CREATE new sub-mailboxes in any
       implementation-defined hierarchy, parent mailbox for the new
       mailbox name in RENAME)
   x - delete mailbox (DELETE mailbox, old mailbox name in RENAME)
   t - delete messages (set or clear \DELETED flag via STORE, set
       \DELETED flag during APPEND/COPY)
   e - perform EXPUNGE and expunge as a part of CLOSE
   a - administer (perform SETACL/DELETEACL/GETACL/LISTRIGHTS)

   // RFC2086
   c - create (CREATE new sub-mailboxes in any implementation-defined
       hierarchy)
   d - delete (STORE DELETED flag, perform EXPUNGE)

*/

use std::fmt::Display;

use jmap_client::{mailbox::MailboxRights, principal::ACL};

use crate::core::utf7::utf7_encode;

use super::quoted_string;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Rights {
    Lookup,
    Read,
    Seen,
    Write,
    Insert,
    Post,
    CreateMailbox,
    DeleteMailbox,
    DeleteMessages,
    Expunge,
    Administer,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModRights {
    pub op: ModRightsOp,
    pub rights: Vec<Rights>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ModRightsOp {
    Add,
    Remove,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    pub tag: String,
    pub mailbox_name: String,
    pub identifier: Option<String>,
    pub mod_rights: Option<ModRights>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetAclResponse {
    pub mailbox_name: String,
    pub permissions: Vec<(String, Vec<Rights>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRightsResponse {
    pub mailbox_name: String,
    pub identifier: String,
    pub permissions: Vec<Vec<Rights>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyRightsResponse {
    pub mailbox_name: String,
    pub rights: Vec<Rights>,
}

impl GetAclResponse {
    pub fn into_bytes(self, is_rev2: bool) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.mailbox_name.len() + 10 * self.permissions.len() * 5);
        buf.extend_from_slice(b"* ACL ");
        if is_rev2 {
            quoted_string(&mut buf, &self.mailbox_name);
        } else {
            quoted_string(&mut buf, &utf7_encode(&self.mailbox_name));
        }
        for (identifier, rights) in self.permissions {
            buf.extend_from_slice(b" ");
            quoted_string(&mut buf, &identifier);
            buf.extend_from_slice(b" ");

            for right in rights {
                buf.push(right.to_char());
            }
        }
        buf.extend_from_slice(b"\r\n");
        buf
    }
}

impl ListRightsResponse {
    pub fn into_bytes(self, is_rev2: bool) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            self.mailbox_name.len() + self.identifier.len() + 10 * self.permissions.len() * 5,
        );
        buf.extend_from_slice(b"* LISTRIGHTS ");
        if is_rev2 {
            quoted_string(&mut buf, &self.mailbox_name);
        } else {
            quoted_string(&mut buf, &utf7_encode(&self.mailbox_name));
        }
        buf.extend_from_slice(b" ");
        quoted_string(&mut buf, &self.identifier);
        for rights in self.permissions {
            buf.extend_from_slice(b" ");
            for right in rights {
                buf.push(right.to_char());
            }
        }
        buf.extend_from_slice(b"\r\n");
        buf
    }
}

impl MyRightsResponse {
    pub fn into_bytes(self, is_rev2: bool) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.mailbox_name.len() + 10 + self.rights.len());
        buf.extend_from_slice(b"* MYRIGHTS ");
        if is_rev2 {
            quoted_string(&mut buf, &self.mailbox_name);
        } else {
            quoted_string(&mut buf, &utf7_encode(&self.mailbox_name));
        }
        buf.extend_from_slice(b" ");
        for right in self.rights {
            buf.push(right.to_char());
        }
        buf.extend_from_slice(b"\r\n");
        buf
    }
}

impl Rights {
    pub fn from_acl(value: ACL) -> (Self, Option<Self>) {
        match value {
            ACL::Read => (Rights::Lookup, None),
            ACL::Modify => (Rights::CreateMailbox, None),
            ACL::Delete => (Rights::DeleteMailbox, None),
            ACL::ReadItems => (Rights::Read, None),
            ACL::AddItems => (Rights::Insert, None),
            ACL::ModifyItems => (Rights::Write, Rights::Seen.into()),
            ACL::RemoveItems => (Rights::DeleteMessages, Rights::Expunge.into()),
            ACL::CreateChild => (Rights::CreateMailbox, None),
            ACL::Administer => (Rights::Administer, None),
            ACL::Submit => (Rights::Post, None),
        }
    }

    pub fn into_acl(self) -> ACL {
        match self {
            Rights::Lookup => ACL::Read,
            Rights::Read => ACL::ReadItems,
            Rights::Seen => ACL::ModifyItems,
            Rights::Write => ACL::ModifyItems,
            Rights::Insert => ACL::AddItems,
            Rights::Post => ACL::Submit,
            Rights::CreateMailbox => ACL::CreateChild,
            Rights::DeleteMailbox => ACL::Delete,
            Rights::DeleteMessages => ACL::RemoveItems,
            Rights::Expunge => ACL::RemoveItems,
            Rights::Administer => ACL::Administer,
        }
    }

    pub fn to_char(&self) -> u8 {
        match self {
            Rights::Lookup => b'l',
            Rights::Read => b'r',
            Rights::Seen => b's',
            Rights::Write => b'w',
            Rights::Insert => b'i',
            Rights::Post => b'p',
            Rights::CreateMailbox => b'k',
            Rights::DeleteMailbox => b'x',
            Rights::DeleteMessages => b't',
            Rights::Expunge => b'e',
            Rights::Administer => b'a',
        }
    }
}

impl Display for Rights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rights::Lookup => write!(f, "l"),
            Rights::Read => write!(f, "r"),
            Rights::Seen => write!(f, "s"),
            Rights::Write => write!(f, "w"),
            Rights::Insert => write!(f, "i"),
            Rights::Post => write!(f, "p"),
            Rights::CreateMailbox => write!(f, "k"),
            Rights::DeleteMailbox => write!(f, "x"),
            Rights::DeleteMessages => write!(f, "t"),
            Rights::Expunge => write!(f, "e"),
            Rights::Administer => write!(f, "a"),
        }
    }
}

pub trait AsImapRights {
    fn as_imap_rights(&self) -> Vec<Rights>;
}

impl AsImapRights for MailboxRights {
    fn as_imap_rights(&self) -> Vec<Rights> {
        let mut rights = Vec::with_capacity(5);
        if self.may_read_items() {
            rights.push(Rights::Read);
            rights.push(Rights::Lookup);
        }
        if self.may_add_items() {
            rights.push(Rights::Insert);
        }
        if self.may_remove_items() {
            rights.push(Rights::DeleteMessages);
            rights.push(Rights::Expunge);
        }
        if self.may_set_seen() {
            rights.push(Rights::Seen);
        }
        if self.may_set_keywords() {
            rights.push(Rights::Write);
        }
        if self.may_create_child() {
            rights.push(Rights::CreateMailbox);
        }
        if self.may_rename() {
            rights.push(Rights::DeleteMailbox);
        }
        if self.may_delete() {
            rights.push(Rights::DeleteMailbox);
        }
        if self.may_submit() {
            rights.push(Rights::Post);
        }
        rights
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::acl::{GetAclResponse, ListRightsResponse, MyRightsResponse, Rights};

    #[test]
    fn serialize_acl() {
        assert_eq!(
            String::from_utf8(
                GetAclResponse {
                    mailbox_name: "INBOX".to_string(),
                    permissions: vec![
                        (
                            "Fred".to_string(),
                            vec![
                                Rights::Lookup,
                                Rights::Read,
                                Rights::Seen,
                                Rights::Write,
                                Rights::Insert,
                                Rights::CreateMailbox,
                                Rights::DeleteMessages,
                                Rights::Administer,
                            ]
                        ),
                        (
                            "David".to_string(),
                            vec![
                                Rights::CreateMailbox,
                                Rights::DeleteMessages,
                                Rights::Administer,
                            ]
                        )
                    ]
                }
                .into_bytes(true)
            )
            .unwrap(),
            "* ACL \"INBOX\" \"Fred\" lrswikta \"David\" kta\r\n"
        );

        assert_eq!(
            String::from_utf8(
                ListRightsResponse {
                    mailbox_name: "Deleted Items".to_string(),
                    identifier: "Fred".to_string(),
                    permissions: vec![
                        vec![Rights::Lookup, Rights::Read],
                        vec![Rights::Administer],
                        vec![Rights::DeleteMailbox]
                    ]
                }
                .into_bytes(true)
            )
            .unwrap(),
            "* LISTRIGHTS \"Deleted Items\" \"Fred\" lr a x\r\n"
        );

        assert_eq!(
            String::from_utf8(
                MyRightsResponse {
                    mailbox_name: "Important".to_string(),
                    rights: vec![Rights::Lookup, Rights::Read, Rights::DeleteMailbox]
                }
                .into_bytes(true)
            )
            .unwrap(),
            "* MYRIGHTS \"Important\" lrx\r\n"
        );
    }
}
