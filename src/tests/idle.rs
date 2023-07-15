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

use super::{AssertResult, ImapConnection, Type};

pub async fn test(imap: &mut ImapConnection, imap_check: &mut ImapConnection) {
    // Switch connection to IDLE mode
    imap_check.send("CREATE Parmeggiano").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check.send("SELECT Parmeggiano").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check.send("NOOP").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check.send("IDLE").await;
    imap_check
        .assert_read(Type::Continuation, ResponseType::Ok)
        .await;

    // Expect a new mailbox update
    imap.send("CREATE Provolone").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("LIST () \"/\" \"Provolone\"");

    // Insert a message in the new folder and expect an update
    let message = "From: test@domain.com\nSubject: Test\n\nTest message\n";
    imap.send(&format!("APPEND Provolone {{{}}}", message.len()))
        .await;
    imap.assert_read(Type::Continuation, ResponseType::Ok).await;
    imap.send_untagged(message).await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("STATUS \"Provolone\"")
        .assert_contains("MESSAGES 1")
        .assert_contains("UNSEEN 1")
        .assert_contains("UIDNEXT 2");

    // Change message to Seen and expect an update
    imap.send("SELECT Provolone").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap.send("STORE 1:* +FLAGS (\\Seen)").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("STATUS \"Provolone\"")
        .assert_contains("MESSAGES 1")
        .assert_contains("UNSEEN 0")
        .assert_contains("UIDNEXT 2");

    // Delete message and expect an update
    imap.send("STORE 1:* +FLAGS (\\Deleted)").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap.send("CLOSE").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("STATUS \"Provolone\"")
        .assert_contains("MESSAGES 0")
        .assert_contains("UNSEEN 0")
        .assert_contains("UIDNEXT 2");

    // Delete folder and expect an update
    imap.send("DELETE Provolone").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("LIST (\\NonExistent) \"/\" \"Provolone\"");

    // Add a message to Inbox and expect an update
    imap.send(&format!("APPEND Parmeggiano {{{}}}", message.len()))
        .await;
    imap.assert_read(Type::Continuation, ResponseType::Ok).await;
    imap.send_untagged(message).await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("MESSAGES 1")
        .assert_contains("UNSEEN 1");
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("* 1 EXISTS");
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("* 1 FETCH (FLAGS () UID 1)");

    // Delete message and expect an update
    imap.send("SELECT Parmeggiano").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap.send("STORE 1 +FLAGS (\\Deleted)").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("* 1 FETCH (FLAGS (\\Deleted) UID 1)");

    imap.send("UID EXPUNGE").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("* 1 EXPUNGE")
        .assert_contains("* 0 EXISTS");
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("MESSAGES 0")
        .assert_contains("UNSEEN 0");
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("* 1 EXPUNGE");
    imap_check
        .assert_read(Type::Status, ResponseType::Ok)
        .await
        .assert_contains("* 0 EXISTS");

    // Stop IDLE mode
    imap_check.send_raw("DONE").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_check.send("NOOP").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;
}
