//! The server-sent events of a response body.

use std::io::{BufRead, Error as IoError};
use std::mem;

use typed_builder::TypedBuilder;

/// A server-sent event.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SseEvent {
    /// The name of the event.
    pub name: Option<String>,
    /// The data of the event.
    pub data: String,
}

/// The server-sent events of a response body.
#[derive(TypedBuilder)]
pub struct SseEvents<R: BufRead> {
    /// The body which the events are read from.
    reader: R,
    /// The line which was read most recently, kept so that it is not reallocated for every line.
    #[builder(default)]
    line: String,
    /// The name of the event being read.
    #[builder(default)]
    name: Option<String>,
    /// The data of the event being read.
    #[builder(default)]
    data: String,
    /// Whether any field of the event being read has been seen.
    #[builder(default)]
    started: bool,
}

impl<R: BufRead> SseEvents<R> {
    /// Return the event which has been read and start reading the next one.
    ///
    /// Events which have no fields are not dispatched, so that the blank lines between events do
    /// not each produce one.
    fn take(&mut self) -> Option<SseEvent> {
        if !self.started {
            return None;
        }

        self.started = false;
        return Some(SseEvent {
            name: self.name.take(),
            data: mem::take(&mut self.data),
        });
    }
}

impl<R: BufRead> Iterator for SseEvents<R> {
    type Item = Result<SseEvent, IoError>;

    fn next(&mut self) -> Option<Result<SseEvent, IoError>> {
        loop {
            self.line.clear();
            let read: usize = match self.reader.read_line(&mut self.line) {
                Ok(read) => read,
                Err(error) => return Some(Err(error)),
            };

            // The body ended. An event which was never terminated by a blank line is still
            // dispatched, since there is nothing more coming which could terminate it.
            if read == 0 {
                return self.take().map(Ok);
            }

            let line: &str = self.line.trim_end_matches(['\n', '\r']);

            if line.is_empty() {
                match self.take() {
                    Some(event) => return Some(Ok(event)),
                    None => continue,
                }
            }

            // A line which starts with a colon is a comment. Inference engines send these to keep
            // the connection from going idle.
            if line.starts_with(':') {
                continue;
            }

            // A line with no colon at all is a field with an empty value.
            let (field, value): (&str, &str) = match line.split_once(':') {
                Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
                None => (line, ""),
            };

            match field {
                "event" => {
                    self.name = Some(value.to_string());
                    self.started = true;
                }
                "data" => {
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data.push_str(value);
                    self.started = true;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::BufReader;

    use test_case::test_case;

    /// Return the events of a body.
    fn events(body: &str) -> Vec<SseEvent> {
        SseEvents::builder()
            .reader(BufReader::new(body.as_bytes()))
            .build()
            .map(|event| event.unwrap())
            .collect()
    }

    /// Return an event.
    fn event(name: Option<&str>, data: &str) -> SseEvent {
        SseEvent {
            name: name.map(str::to_string),
            data: data.to_string(),
        }
    }

    #[test_case("data: one\n\n", vec![event(None, "one")]; "one data field")]
    #[test_case("data:one\n\n", vec![event(None, "one")]; "without a space after the colon")]
    #[test_case(
        "event: delta\ndata: one\n\n",
        vec![event(Some("delta"), "one")];
        "a named event"
    )]
    #[test_case(
        "data: one\ndata: two\n\n",
        vec![event(None, "one\ntwo")];
        "data fields are joined with newlines"
    )]
    #[test_case(
        "data: one\n\ndata: two\n\n",
        vec![event(None, "one"), event(None, "two")];
        "two events"
    )]
    #[test_case("data: one\r\n\r\n", vec![event(None, "one")]; "carriage returns")]
    #[test_case(": ping\n\ndata: one\n\n", vec![event(None, "one")]; "comments are skipped")]
    #[test_case("\n\n\n", vec![]; "blank lines alone do not make events")]
    #[test_case("data: one", vec![event(None, "one")]; "an event not terminated by a blank line")]
    #[test_case("", vec![]; "an empty body")]
    #[test_case("data: [DONE]\n\n", vec![event(None, "[DONE]")]; "the done marker")]
    #[test_case("data\n\n", vec![event(None, "")]; "a field with no colon")]
    #[test_case(
        "id: 1\ndata: one\n\n",
        vec![event(None, "one")];
        "fields which are not used are ignored"
    )]
    fn test_events(body: &str, expected: Vec<SseEvent>) {
        assert_eq!(events(body), expected);
    }

    #[test]
    fn test_frame_split_across_reads() {
        // `BufReader` with a tiny buffer makes `read_line` fill more than once for a single line,
        // which is what happens when the network delivers a frame in pieces.
        let body: &str = "event: delta\ndata: {\"text\":\"hello there\"}\n\n";
        let events: Vec<SseEvent> = SseEvents::builder()
            .reader(BufReader::with_capacity(16, body.as_bytes()))
            .build()
            .map(|event| event.unwrap())
            .collect();

        assert_eq!(
            events,
            vec![event(Some("delta"), "{\"text\":\"hello there\"}")]
        );
    }
}
