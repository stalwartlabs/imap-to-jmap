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
    // Searches without selecting a mailbox should fail.
    imap.send("SEARCH RETURN (MIN MAX COUNT ALL) ALL").await;
    imap.assert_read(Type::Tagged, ResponseType::Bad).await;

    // Select INBOX
    imap.send("SELECT INBOX").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("10 EXISTS")
        .assert_contains("[UIDNEXT 11]");
    imap_check.send("SELECT INBOX").await;
    imap_check.assert_read(Type::Tagged, ResponseType::Ok).await;

    // Min, Max and Count
    imap.send("SEARCH RETURN (MIN MAX COUNT ALL) ALL").await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("COUNT 10 MIN 1 MAX 10 ALL 1,10");
    imap_check.send("UID SEARCH ALL").await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 1 2 3 4 5 6 7 8 9 10");

    // Filters
    imap_check
        .send("UID SEARCH OR FROM nathaniel SUBJECT argentina")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 1 3 4 6");

    imap_check
        .send("UID SEARCH UNSEEN OR KEYWORD Flag_007 KEYWORD Flag_004")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 5 8");

    imap_check
        .send("UID SEARCH TEXT coffee FROM vandelay SUBJECT exporting SENTON 20-Nov-2021")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 10");

    imap_check
        .send("UID SEARCH NOT (FROM nathaniel ANSWERED)")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 2 3 5 7 8 9 10");

    imap_check
        .send("UID SEARCH UID 0:6 LARGER 1000 SMALLER 2000")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 1 2");

    // Saved search
    imap_check.send(
        "UID SEARCH RETURN (SAVE ALL) OR OR FROM nathaniel FROM vandelay OR SUBJECT rfc FROM gore",
    )
    .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("1,3:4,6,8,10");

    imap_check.send("UID SEARCH NOT $").await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 2 5 7 9");

    imap_check
        .send("UID SEARCH $ SMALLER 1000 SUBJECT section")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SEARCH 8");

    imap_check.send("UID SEARCH RETURN (MIN MAX) NOT $").await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("MIN 2 MAX 9");

    // Sort
    imap_check
        .send("UID SORT (REVERSE SUBJECT REVERSE DATE) UTF-8 FROM Nathaniel")
        .await;
    imap_check
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_equals("* SORT 6 4 1");

    imap.send("UID SORT RETURN (COUNT ALL) (DATE SUBJECT) UTF-8 ALL")
        .await;
    imap.assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains("COUNT 10 ALL 6,4:5,1,3,7:8,10,2,9");
}
