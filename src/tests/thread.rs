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

use crate::core::ResponseType;

use super::{append::build_messages, AssertResult, ImapConnection, Type};

pub async fn test(imap: &mut ImapConnection, _imap_check: &mut ImapConnection) {
    // Create test messages
    let messages = build_messages();

    // Insert messages using Multiappend
    imap.send("CREATE Manchego").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    for (pos, message) in messages.iter().enumerate() {
        if pos == 0 {
            imap.send(&format!("APPEND Manchego {{{}}}", message.len()))
                .await;
        } else {
            imap.send_untagged(&format!(" {{{}}}", message.len())).await;
        }
        imap.assert_read(Type::Continuation, ResponseType::Ok).await;
        if pos < messages.len() - 1 {
            imap.send_raw(message).await;
        } else {
            imap.send_untagged(message).await;
            assert_eq!(
                imap.assert_read(Type::Tagged, ResponseType::Ok)
                    .await
                    .into_append_uid(),
                format!("1:{}", messages.len()),
            );
        }
    }

    // Obtain ThreadId and MessageId of the first message
    imap.send("SELECT Manchego").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;

    let mut email_id = None;
    let mut thread_id = None;
    imap.send("UID FETCH 1 (EMAILID THREADID)").await;
    for line in imap.assert_read(Type::Tagged, ResponseType::Ok).await {
        if let Some((_, value)) = line.split_once("EMAILID (") {
            email_id = value
                .split_once(')')
                .expect("Missing delimiter")
                .0
                .to_string()
                .into();
        }
        if let Some((_, value)) = line.split_once("THREADID (") {
            thread_id = value
                .split_once(')')
                .expect("Missing delimiter")
                .0
                .to_string()
                .into();
        }
    }
    let email_id = email_id.expect("Missing EMAILID");
    let thread_id = thread_id.expect("Missing THREADID");

    // 4 different threads are expected
    imap.send("THREAD REFERENCES UTF-8 1:*").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("(1 2 3 4)")
        .assert_contains("(5 6 7 8)")
        .assert_contains("(9 10 11 12)");

    imap.send("THREAD REFERENCES UTF-8 SUBJECT T1").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("(5 6 7 8)")
        .assert_count("(1 2 3 4)", 0)
        .assert_count("(9 10 11 12)", 0);

    // Filter by threadId and messageId
    imap.send(&format!(
        "UID THREAD REFERENCES UTF-8 THREADID {}",
        thread_id
    ))
    .await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("(1 2 3 4)")
        .assert_count("(", 1);

    imap.send(&format!("UID THREAD REFERENCES UTF-8 EMAILID {}", email_id))
        .await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("(1)")
        .assert_count("(", 1);

    // Delete all messages
    imap.send("STORE 1:* +FLAGS.SILENT (\\Deleted)").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap.send("EXPUNGE").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("EXPUNGE", 13);
}
