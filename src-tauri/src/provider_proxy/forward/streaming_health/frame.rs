use super::{inspect_frame, StreamHealth};

const MAX_FRAME: usize = 1024 * 1024;
const MAX_LINE_PREFIX: usize = 256;

#[derive(Default)]
pub(super) struct Inspector {
    frame: Vec<u8>,
    oversized: bool,
    line: Vec<u8>,
    long_line: bool,
    event: Option<String>,
    has_data: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_lines_and_frames_keep_parser_storage_bounded() {
        let mut parser = Inspector::default();
        let health = StreamHealth::requiring_completion();
        for _ in 0..3 * MAX_FRAME {
            parser.push(b'x', &health);
        }
        assert!(parser.frame.len() <= MAX_FRAME);
        assert!(parser.frame.capacity() <= MAX_FRAME);
        assert!(parser.line.len() <= MAX_LINE_PREFIX);
        assert!(parser.line.capacity() <= MAX_LINE_PREFIX);
        assert!(parser.oversized);
        parser.finish_eof(&health);
        assert!(health.incomplete());
    }
}

impl Inspector {
    pub(super) fn push(&mut self, byte: u8, health: &StreamHealth) {
        if !self.oversized {
            if self.frame.len() < MAX_FRAME {
                self.frame.push(byte);
            } else {
                self.oversized = true;
                self.frame.clear();
                health.unknown_frame();
            }
        }
        if byte == b'\n' {
            let line = self.line.strip_suffix(b"\r").unwrap_or(&self.line);
            if !self.long_line && line.is_empty() {
                self.finish_event(health);
            } else {
                if let Some(name) = line.strip_prefix(b"event:") {
                    // The last event field wins, including unknown/oversized names.
                    self.event =
                        (!self.long_line).then(|| String::from_utf8_lossy(name).trim().to_string());
                }
                self.has_data |= line.starts_with(b"data:");
            }
            self.line.clear();
            self.long_line = false;
        } else if self.line.len() < MAX_LINE_PREFIX {
            self.line.push(byte);
        } else {
            self.long_line = true;
        }
    }

    fn finish_event(&mut self, health: &StreamHealth) {
        if !self.oversized {
            inspect_frame(&self.frame, health);
        } else if self.has_data {
            // The event header remains bounded even when image/tool output is huge.
            // Completion is accepted only after the event's terminating blank line.
            match self.event.as_deref() {
                Some("message_stop" | "response.completed" | "response.incomplete") => {
                    health.completed()
                }
                Some("error" | "response.failed") => health.fail(),
                _ => {}
            }
        }
        self.frame.clear();
        self.oversized = false;
        self.event = None;
        self.has_data = false;
    }

    pub(super) fn finish_eof(&self, health: &StreamHealth) {
        // A large event cut off inside its payload never confirms completion.
        if !self.oversized {
            inspect_frame(&self.frame, health);
        }
    }
}
