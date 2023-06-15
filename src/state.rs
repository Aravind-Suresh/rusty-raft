use std::fs::{File, read_to_string};
use std::io::Write;
use std::io::ErrorKind;
use serde::{Serialize, Deserialize};
use std::fmt; 
use crate::config::{Config};

fn int_default() -> u32 {
    return 0
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogEntry {
    pub content: String,
    pub term: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Mode {
    Leader,
    Follower,
    Candidate,
}

fn mode_default() -> Mode {
    Mode::Candidate
}

#[derive(Serialize, Deserialize, Debug)]
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
    pub last_heartbeat_recv_millis: u128,
    #[serde(skip_serializing, default)]
    pub leader_id: u32,

    // volatile state for leaders
    #[serde(skip_serializing, default="Vec::new")]
    pub next_index: Vec<u32>,
    #[serde(skip_serializing, default="Vec::new")]
    pub match_index: Vec<u32>,
    #[serde(skip_serializing, default)]
    pub last_heartbeat_sent_millis: u128,
}

impl State {
    pub fn persist(self: &Self) {
        let mut out = File::create(format!("state/{}.bin", self.id)).unwrap();
        let serialized = serde_json::to_string(self).unwrap();
        out.write(serialized.as_bytes()).unwrap();
    }
    pub fn restore(id: u32, cfg: &Config) -> Self {
        let res = read_to_string(format!("state/{}.bin", id));
        let inp = match res {
            Ok(inp) => inp,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => "{\"id\":".to_owned() + &id.to_string() + 
                    ",\"current_term\":0,\"voted_for\":0,\"logs\":[]}",
                other_error => panic!("unexpected error: {:?}", other_error),
            }
        };
        let mut state = serde_json::from_str::<Self>(&inp).unwrap();
        state.commit_index = 0;
        state.last_applied = 0;
        state.mode = Mode::Follower;
        state.last_heartbeat_recv_millis = 0;
        state.leader_id = 0;
        state.next_index = vec![1; cfg.nodes.len()];
        state.match_index = vec![0; cfg.nodes.len()];
        state.last_heartbeat_sent_millis = 0;
        state.persist();
        return state;
    }
    pub fn reset_leader_state(&mut self) {
        for i in &mut self.next_index {
            *i = 1;
        }
        for i in &mut self.match_index {
            *i = 0;
        }
    }
    pub fn derive_commit_index(&self, cfg: &Config) -> u32 {
        let majority = cfg.majority_vote_count();
        let mut vec = self.next_index.clone();
        vec.sort();
        vec.reverse();
        let idx = (majority - 1) as usize;
        if idx >= vec.len() {
            return 0;
        }
        return vec[idx];
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
