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

use crate::{
    core::{client::Session, receiver::Request, Command, StatusResponse},
    protocol::{capability::Capability, ProtocolVersion},
};

impl Session {
    pub async fn handle_enable(&mut self, request: Request<Command>) -> Result<(), ()> {
        match request.parse_enable() {
            Ok(arguments) => {
                for capability in arguments.capabilities {
                    match capability {
                        Capability::IMAP4rev2 => {
                            self.version = ProtocolVersion::Rev2;
                        }
                        Capability::IMAP4rev1 => {
                            self.version = ProtocolVersion::Rev1;
                        }
                        Capability::CondStore => {
                            self.is_condstore = true;
                        }
                        Capability::QResync => {
                            self.is_qresync = true;
                        }
                        Capability::Utf8Accept => {}
                        _ => {
                            let mut buf = Vec::with_capacity(10);
                            capability.serialize(&mut buf);
                            self.write_bytes(
                                StatusResponse::ok(format!(
                                    "{} cannot be enabled.",
                                    String::from_utf8(buf).unwrap()
                                ))
                                .with_tag(arguments.tag)
                                .into_bytes(),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                }

                self.write_bytes(
                    StatusResponse::ok("ENABLE successful.")
                        .with_tag(arguments.tag)
                        .into_bytes(),
                )
                .await
            }
            Err(response) => self.write_bytes(response.into_bytes()).await,
        }
    }
}
