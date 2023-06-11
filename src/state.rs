use std::fs::{File, read_to_string};
use std::io::Write;
use std::io::ErrorKind;
use serde::{Serialize, Deserialize};

fn int_default() -> u32 {
    return 0
}

#[derive(Serialize, Deserialize)]
pub struct LogEntry {
    pub content: String,
    pub term: u32,
}

#[derive(Serialize, Deserialize)]
pub enum Mode {
    Leader,
    Follower,
    Candidate,
}

fn mode_default() -> Mode {
    Mode::Candidate
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub id: u32,
    pub current_term: u32,
    pub voted_for: u32,
    pub logs: Vec<LogEntry>,

    // common volatile state
    #[serde(skip_serializing, default="int_default")]
    pub commit_index: u32,
    #[serde(skip_serializing, default="int_default")]
    pub last_applied: u32,
    #[serde(skip_serializing, default="mode_default")]
    pub mode: Mode,
    #[serde(skip_serializing, default)]
    pub last_heartbeat_at_millis: u128,

    // volatile state for leaders
    #[serde(skip_serializing, default="Vec::new")]
    pub next_index: Vec<u32>,
    #[serde(skip_serializing, default="Vec::new")]
    pub match_index: Vec<u32>,
}

impl State {
    pub fn persist(self: &Self) -> String {
        let mut out = File::create("state/persisted.bin").unwrap();
        let serialized = serde_json::to_string(self).unwrap();
        out.write(serialized.as_bytes()).unwrap();
        serialized
    }
    pub fn restore(id: u32) -> Self {
        let res = read_to_string("state/persisted.bin");
        let inp = match res {
            Ok(inp) => inp,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => {
                    let state = State {
                        id: id,
                        current_term: 0,
                        voted_for: 0,
                        logs: Vec::new(),
                        commit_index: 0,
                        last_applied: 0,
                        mode: Mode::Follower,
                        last_heartbeat_at_millis: 0,
                        next_index: Vec::new(),
                        match_index: Vec::new(),
                    };
                    state.persist()
                },
                other_error => panic!("unexpected error: {:?}", other_error),
            }
        };
        let deserialized = serde_json::from_str::<Self>(&inp).unwrap();
        return deserialized;
    }
}
