#[derive(Default)]
pub struct KeyInput {
    state: KeyInputState,
}

#[derive(Default)]
enum KeyInputState {
    #[default]
    Normal,
    Escape,
    Csi,
}

impl KeyInput {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Key> {
        let mut keys = Vec::new();
        for byte in bytes {
            match self.state {
                KeyInputState::Normal => match byte {
                    b'\x1b' => self.state = KeyInputState::Escape,
                    b'\r' | b'\n' | b'i' => keys.push(Key::AgentMode),
                    b'q' => keys.push(Key::Quit),
                    b'k' => keys.push(Key::Up),
                    b'j' => keys.push(Key::Down),
                    b'G' => keys.push(Key::Bottom),
                    b'g' => keys.push(Key::G),
                    b'r' => keys.push(Key::Refresh),
                    b'R' => keys.push(Key::ReviewPacket),
                    b'f' => keys.push(Key::ReviewFix),
                    b'm' => keys.push(Key::CommitReviewFix),
                    b'u' => keys.push(Key::Push),
                    b'n' => keys.push(Key::CreatePlan),
                    b'x' => keys.push(Key::RunPlan),
                    b'P' => keys.push(Key::PullRequest),
                    b'c' => keys.push(Key::Create),
                    b'a' => keys.push(Key::Remove),
                    b'D' => keys.push(Key::Delete),
                    _ => keys.push(Key::Other),
                },
                KeyInputState::Escape => {
                    if *byte == b'[' {
                        self.state = KeyInputState::Csi;
                    } else {
                        self.state = KeyInputState::Normal;
                        keys.push(Key::Other);
                    }
                }
                KeyInputState::Csi => {
                    self.state = KeyInputState::Normal;
                    match byte {
                        b'A' => keys.push(Key::Up),
                        b'B' => keys.push(Key::Down),
                        _ => keys.push(Key::Other),
                    }
                }
            }
        }
        keys
    }
}

pub enum Key {
    Up,
    Down,
    Bottom,
    G,
    AgentMode,
    Refresh,
    PullRequest,
    ReviewPacket,
    ReviewFix,
    CommitReviewFix,
    Push,
    CreatePlan,
    RunPlan,
    Create,
    Remove,
    Delete,
    Quit,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_input_handles_batched_keys() {
        let mut input = KeyInput::default();
        let keys = input.feed(b"jq");
        assert!(matches!(keys.as_slice(), [Key::Down, Key::Quit]));
    }

    #[test]
    fn key_input_handles_agent_mode_keys() {
        let mut input = KeyInput::default();
        let keys = input.feed(b"i\n");
        assert!(matches!(keys.as_slice(), [Key::AgentMode, Key::AgentMode]));
    }

    #[test]
    fn key_input_handles_cleanup_keys() {
        let mut input = KeyInput::default();
        let keys = input.feed(b"aD");
        assert!(matches!(keys.as_slice(), [Key::Remove, Key::Delete]));
    }
}
