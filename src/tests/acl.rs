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

use super::{append::assert_append_message, AssertResult, ImapConnection, Type};

pub async fn test(mut imap_john: &mut ImapConnection, _imap_check: &mut ImapConnection) {
    // Connect to all test accounts
    let mut imap_jane = ImapConnection::connect(b"_w ").await;
    let mut imap_bill = ImapConnection::connect(b"_z ").await;
    for (imap, secret) in [
        (&mut imap_jane, "AGphbmUuc21pdGhAZXhhbXBsZS5jb20Ac2VjcmV0"),
        (&mut imap_bill, "AGZvb2JhckBleGFtcGxlLmNvbQBzZWNyZXQ="),
    ] {
        imap.assert_read(Type::Untagged, ResponseType::Ok).await;
        imap.send(&format!(
            "AUTHENTICATE PLAIN {{{}+}}\r\n{}",
            secret.len(),
            secret
        ))
        .await;
        imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    }

    // John should have no shared folders
    imap_john.send("LIST \"\" \"*\"").await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("Shared Folders", 0);
    imap_john.send("NAMESPACE").await;
    imap_john.assert_read(Type::Tagged, ResponseType::Ok).await;

    // List rights
    imap_jane.send("LISTRIGHTS INBOX jdoe@example.com").await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* LISTRIGHTS \"INBOX\" \"jdoe@example.com\" r l ws i et k x p a");

    // Jane shares her Inbox to John, expect a Shared Folders item in John's list
    imap_jane.send("SETACL INBOX jdoe@example.com lr").await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_john.send("LIST \"\" \"*\"").await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* LIST (\\NoSelect) \"/\" \"Shared Folders\"")
        .assert_equals("* LIST (\\NoSelect) \"/\" \"Shared Folders/Jane Smith\"")
        .assert_equals("* LIST () \"/\" \"Shared Folders/Jane Smith/Inbox\"");

    // Grant access to Bill and check ACLs
    imap_jane.send("GETACL INBOX").await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("\"jdoe@example.com\" rl");

    imap_jane
        .send("SETACL INBOX foobar@example.com lrxtws")
        .await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_jane.send("GETACL INBOX").await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("\"jdoe@example.com\" rl")
        .assert_contains("\"foobar@example.com\" tewsrxl");

    imap_bill.send("LIST \"\" \"*\"").await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("Shared Folders/Jane Smith/Inbox");

    // Namespace should now return the Shared Folders namespace
    imap_john.send("NAMESPACE").await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* NAMESPACE ((\"\" \"/\")) ((\"Shared Folders\" \"/\")) NIL");

    // List John's right on Jane's Inbox
    imap_john
        .send("MYRIGHTS \"Shared Folders/Jane Smith/Inbox\"")
        .await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* MYRIGHTS \"Shared Folders/Jane Smith/Inbox\" rl");

    // John should not be able to append messages
    assert_append_message(
        imap_john,
        "Shared Folders/Jane Smith/Inbox",
        "From: john\n\ncontents",
        ResponseType::No,
    )
    .await;

    // Grant insert access to John on Jane's Inbox, and try inserting the
    // message again.
    imap_jane.send("SETACL INBOX jdoe@example.com +i").await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;
    imap_john
        .send("MYRIGHTS \"Shared Folders/Jane Smith/Inbox\"")
        .await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* MYRIGHTS \"Shared Folders/Jane Smith/Inbox\" rli");
    assert_append_message(
        imap_john,
        "Shared Folders/Jane Smith/Inbox",
        "From: john\n\ncontents",
        ResponseType::Ok,
    )
    .await;

    // Only Bill should be allowed to delete messages on Jane's Inbox
    for imap in [&mut imap_john, &mut imap_bill] {
        imap.send("SELECT \"Shared Folders/Jane Smith/Inbox\"")
            .await;
        imap.assert_read(Type::Tagged, ResponseType::Ok).await;
    }
    imap_john.send("UID STORE 1 +FLAGS (\\Deleted)").await;
    imap_john.assert_read(Type::Tagged, ResponseType::No).await;

    imap_bill.send("UID STORE 1 +FLAGS (\\Deleted)").await;
    imap_bill.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_john.send("UID EXPUNGE").await;
    imap_john.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_john.send("UID FETCH 1 (PREVIEW)").await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("contents");

    imap_bill.send("UID EXPUNGE").await;
    imap_bill.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_bill.send("UID FETCH 1 (PREVIEW)").await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("contents", 0);

    imap_bill
        .send("STATUS \"Shared Folders/Jane Smith/Inbox\" (MESSAGES)")
        .await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("(MESSAGES 0)");

    // Test copying and moving between shared mailboxes
    let uid = assert_append_message(
        imap_john,
        "INBOX",
        "From: john\n\ncopy test",
        ResponseType::Ok,
    )
    .await
    .into_append_uid();

    imap_john.send("SELECT INBOX").await;
    imap_john.assert_read(Type::Tagged, ResponseType::Ok).await;

    // Copy from John's Inbox to Jane's Inbox
    imap_john
        .send(&format!(
            "UID COPY {} \"Shared Folders/Jane Smith/Inbox\"",
            uid
        ))
        .await;
    let uid = imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .into_copy_uid();

    // Check that both Bill and Jane can see the message
    imap_bill.send("NOOP").await;
    imap_bill.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_bill
        .send(&format!("UID FETCH {} (PREVIEW)", uid))
        .await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("copy test");

    imap_jane.send("SELECT INBOX").await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_jane
        .send(&format!("UID FETCH {} (PREVIEW)", uid))
        .await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("copy test");

    // Bill now moves the message to his own Inbox
    imap_bill.send(&format!("UID MOVE {} INBOX", uid)).await;
    let uid_moved = imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .into_copy_uid();

    // Both Jane and Bill should not see the message on Jane's Inbox anymore
    imap_bill
        .send(&format!("UID FETCH {} (PREVIEW)", uid))
        .await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("copy test", 0);

    imap_jane
        .send(&format!("UID FETCH {} (PREVIEW)", uid))
        .await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("copy test", 0);

    // Check that the message has been moved to Bill's Inbox
    imap_bill.send("SELECT INBOX").await;
    imap_bill.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_bill
        .send(&format!("UID FETCH {} (PREVIEW)", uid_moved))
        .await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("copy test");

    // Jane stops sharing with Bill, and removes Insert access to John
    imap_jane.send("DELETEACL INBOX foobar@example.com").await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_jane.send("SETACL INBOX jdoe@example.com -i").await;
    imap_jane.assert_read(Type::Tagged, ResponseType::Ok).await;

    imap_jane.send("GETACL INBOX").await;
    imap_jane
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("\"jdoe@example.com\" rl")
        .assert_count("foobar@example.com", 0);

    // Bill should not have access to Jane's Inbox anymore
    imap_bill.send("LIST \"\" \"*\"").await;
    imap_bill
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("Shared Folders", 0);

    // And John should still have access
    imap_john.send("LIST \"\" \"*\"").await;
    imap_john
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_count("Shared Folders", 3);
}
