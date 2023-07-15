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

use super::{quoted_string, ImapResponse};

pub struct Response {
    pub shared_prefix: Option<String>,
}

impl ImapResponse for Response {
    fn serialize(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);
        if let Some(shared_prefix) = &self.shared_prefix {
            buf.extend_from_slice(b"* NAMESPACE ((\"\" \"/\")) ((");
            quoted_string(&mut buf, shared_prefix);
            buf.extend_from_slice(b" \"/\")) NIL\r\n");
        } else {
            buf.extend_from_slice(b"* NAMESPACE ((\"\" \"/\")) NIL NIL\r\n");
        }
        buf
    }
}
