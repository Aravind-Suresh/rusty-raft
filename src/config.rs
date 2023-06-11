// TODO: refactor Node to Participant
#[derive(Clone)]
pub struct Node {
    pub id: u32,
    pub addr: String,
}

pub struct Config {
    pub nodes: Vec<Node>,
}

impl Config {
    pub fn majority_vote_count(&self) -> u32 {
        let mut ret = self.nodes.len() / 2;
        ret += self.nodes.len() % 2;
        return ret as u32
    }
}