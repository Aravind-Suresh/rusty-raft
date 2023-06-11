pub struct Node {
    id: int,
    addr: String,
}

pub struct Config {
    nodes: Vec<Node>,
}

impl Config {
    fn majority_vote_count(&self) -> u32 {
        let mut ret = nodes.len() / 2;
        ret += nodes.len() % 2;
        return ret
    }
}