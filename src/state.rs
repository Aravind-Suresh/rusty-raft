use std::fs::{File, read_to_string};
use std::io::Write;
use std::io::ErrorKind;
use serde::{Serialize, Deserialize};

fn int_default() -> i32 {
    return -1
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub current_term: i32,
    pub voted_for: i32,
    pub log: Vec<String>,

    // common volatile state
    #[serde(skip_serializing, default="int_default")]
    pub commit_index: i32,
    #[serde(skip_serializing, default="int_default")]
    pub last_applied: i32,

    // volatile state for leaders
    #[serde(skip_serializing, default="Vec::new")]
    pub next_index: Vec<i32>,
    #[serde(skip_serializing, default="Vec::new")]
    pub match_index: Vec<i32>,
}

impl State {
    pub fn persist(self: &Self) -> String {
        let mut out = File::create("state/persisted.bin").unwrap();
        let serialized = serde_json::to_string(self).unwrap();
        out.write(serialized.as_bytes()).unwrap();
        serialized
    }
    pub fn restore() -> Self {
        let res = read_to_string("state/persisted.bin");
        let inp = match res {
            Ok(inp) => inp,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => {
                    let state = State {
                        current_term: -1,
                        voted_for: -1,
                        log: Vec::new(),
                        commit_index: -1,
                        last_applied: -1,
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
