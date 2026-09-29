/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_data::InstantEvent;
use yak_data::SpanEndEvent;
use yak_data::SpanStartEvent;
use yak_data::yak_event;
use yak_events::YakEvent;

#[derive(yak_error::Error, Debug)]
#[yak(tag = InvalidEvent)]
pub enum VisitorError {
    #[error("Sent an event missing one or more fields: `{0:?}`")]
    MissingField(YakEvent),
    #[error("Sent an unexpected Record event: `{0:?}`")]
    UnexpectedRecord(YakEvent),
}

/// Just a simple structure that makes it easier to deal with YakEvent rather than
/// needing to deal with the unpacking of optional fields yourself.
pub enum UnpackedYakEvent<'a> {
    SpanStart(
        &'a YakEvent,
        &'a SpanStartEvent,
        &'a yak_data::span_start_event::Data,
    ),
    SpanEnd(
        &'a YakEvent,
        &'a SpanEndEvent,
        &'a yak_data::span_end_event::Data,
    ),
    Instant(
        &'a YakEvent,
        &'a InstantEvent,
        &'a yak_data::instant_event::Data,
    ),
    UnrecognizedSpanStart(&'a YakEvent, &'a SpanStartEvent),
    UnrecognizedSpanEnd(&'a YakEvent, &'a SpanEndEvent),
    UnrecognizedInstant(&'a YakEvent, &'a InstantEvent),
}

pub fn unpack_event(event: &YakEvent) -> yak_error::Result<UnpackedYakEvent<'_>> {
    match &event.data() {
        yak_event::Data::SpanStart(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedYakEvent::SpanStart(event, v, data)
            } else {
                UnpackedYakEvent::UnrecognizedSpanStart(event, v)
            }
        }),
        yak_event::Data::SpanEnd(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedYakEvent::SpanEnd(event, v, data)
            } else {
                UnpackedYakEvent::UnrecognizedSpanEnd(event, v)
            }
        }),
        yak_event::Data::Instant(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedYakEvent::Instant(event, v, data)
            } else {
                UnpackedYakEvent::UnrecognizedInstant(event, v)
            }
        }),
        yak_event::Data::Record(_) => Err(VisitorError::UnexpectedRecord(event.clone()).into()),
    }
}
