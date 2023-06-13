mod state;
mod clock;
mod config;

use state::{State, Mode, LogEntry};
use tonic::{transport::Server, transport::Channel, Request, Response, Status};
use raft::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse};
use raft::raft_participant_server::{RaftParticipant, RaftParticipantServer};
use raft::raft_participant_client::{RaftParticipantClient};
use std::sync::{Arc, RwLock};
use clock::Clock;
use std::{cmp, thread, time, time::Duration};
use std::mem;
use config::{Config, Node};
use std::collections::{hash_map::Entry, HashMap};
use std::env;
use rand::prelude::*;

const MAIN_LOOP_DELAY: time::Duration = time::Duration::from_secs(1);
const ELECTION_TIMEOUT: time::Duration = time::Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: time::Duration = time::Duration::from_secs(1);

mod raft {
    tonic::include_proto!("raft");
}

pub struct RaftParticipantImpl<C: Clock> {
    state: Arc<RwLock<State>>,
    clock: C,
}

#[tonic::async_trait]
impl<C: Clock + 'static> RaftParticipant for RaftParticipantImpl<C> {
    async fn append_entries(
        &self, 
        request: Request<AppendEntriesRequest>
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        println!("rpc: append entries, arg: {:?}", request.get_ref());
        let mut state = self.state.write().unwrap();
        let request = request.get_ref();
        let response_false = Ok(Response::new(AppendEntriesResponse{
            term: state.current_term,
            success: false,
        }));
        if request.term < state.current_term {
            return response_false
        }
        if state.logs.len() > 0 && state.logs.get(request.prev_log_index as usize).unwrap().term != request.prev_log_term {
            return response_false
        }
        // removing entries that are conflicting with the incoming entries
        // also removes the entries after the prev ping
        while (state.logs.len() as u32) > request.prev_log_index {
            _ = state.logs.pop()
        }
        let current_term = state.current_term;
        // noting down the provided entries
        for entry in request.entries.iter() {
            state.logs.push(LogEntry{
                content: entry.to_string(),
                term: current_term
            })
        }
        if request.leader_commit > state.commit_index {
            state.commit_index = cmp::min(request.leader_commit, state.logs.len() as u32)
        }
        state.last_heartbeat_recv_millis = self.clock.now();
        println!("node:{}.last_heartbeat_recv_millis = {}, from = {}", state.id, state.last_heartbeat_recv_millis, request.leader_id);
        // flushing state to disk before returning the response
        state.persist();
        Ok(Response::new(AppendEntriesResponse{
            term: state.current_term,
            success: true,
        }))
    }

    async fn request_vote(
        &self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        println!("rpc: request vote, arg: {:?}", request.get_ref());
        let mut state = self.state.write().unwrap();
        let request = request.get_ref();
        let response_false = Ok(Response::new(RequestVoteResponse{
            term: state.current_term,
            vote_granted: false,
        }));
        if request.term < state.current_term {
            return response_false
        }
        let can_vote = state.voted_for == 0 || state.voted_for == request.candidate_id;
        if can_vote {
            let candidate_upto_date = request.last_log_term >= state.current_term && request.last_log_index >= (state.logs.len() as u32);
            if candidate_upto_date {
                // recording that this server actually voted for this candidate
                state.voted_for = request.candidate_id;
                // flushing state to disk before returning the response
                state.persist();
                return Ok(Response::new(RequestVoteResponse{
                    term: state.current_term,
                    vote_granted: true,
                }));
            }
        }
        response_false
    }
}

fn jittered_sleep(dur: Duration, id: u32) {
    let min_jitter: u64 = ((dur.as_millis() as f64) * 1.0 * (id as f64)) as u64;
    let max_jitter: u64 = ((dur.as_millis() as f64) * 2.0 * (id as f64)) as u64;
    let mut jitter: u64 = random();
    jitter = min_jitter + jitter % (max_jitter - min_jitter);
    println!("jittered sleep, base: {}, jitter: {}", dur.as_millis(), jitter);
    thread::sleep(dur + Duration::from_millis(jitter));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clock = clock::SystemClock{};
    // TODO: move this to a config file and read it
    // TODO: add config validation checks
    let cfg = Config{
        nodes: [
            Node{
                id: 1,
                addr: "[::1]:50051".to_string(),
            },
            Node {
                id: 2,
                addr: "[::1]:50052".to_string(),
            },
            Node {
                id: 3,
                addr: "[::1]:50053".to_string(),
            }
        ].to_vec()
    };
    // TODO: this entire main function can be moved to a raft.step method
    let mut clients: HashMap<u32, RaftParticipantClient<Channel>> = HashMap::new();

    let args: Vec<String> = env::args().collect();
    let id: u32 = args[1].parse().unwrap();
    let state = Arc::new(RwLock::new(State::restore(id, &cfg)));
    let raft = RaftParticipantImpl{
        state: Arc::clone(&state),
        clock: clock,
    };

    let mut addr: String = "".to_string();
    for node in &cfg.nodes {
        if node.id == id {
            addr = node.addr.to_string()
        }
    }
    if addr == "" {
        panic!("invalid cfg, unable to find current node addr");
    }

    // server started in a different thread
    tokio::spawn(async move {
        let _server = Server::builder()
        .add_service(RaftParticipantServer::new(raft))
        .serve(addr.parse().unwrap())
        .await.unwrap();
        println!("server listening on {}", addr);
    });

    // TODO: remove wait and add handshake with every node
    jittered_sleep(time::Duration::from_secs(3), id);

    println!("creating connections with all nodes");

    for node in &cfg.nodes {
        if node.id == id {
            continue;
        }
        match clients.entry(node.id) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let client_addr = "http://".to_string() + &node.addr.to_string();
                println!("[node:{}] trying to connect to node:{}, {}", id, node.id, client_addr);
                let c = RaftParticipantClient::connect(client_addr).await.unwrap();
                v.insert(c)
            }
        };
    }

    loop {
        // TODO: do optimistic locking here
        let mut state = state.write().unwrap();
        println!("new run of main loop, started at = {}, state = {}", clock.now(), state);
        match state.mode {
            Mode::Follower => {
                if clock.since(state.last_heartbeat_recv_millis) >= ELECTION_TIMEOUT.as_millis() {
                    state.mode = Mode::Candidate;
                }
            },
            Mode::Candidate => {
                // conduct an election here
                println!("started election, node: {}", id);
                state.current_term += 1;
                state.voted_for = state.id;
                state.last_heartbeat_recv_millis = clock.now();

                let mut votes = 0;
                for node in &cfg.nodes {
                    if node.id == id {
                        continue
                    }
                    let client = clients.get_mut(&node.id).unwrap();
                    let mut request = Request::new(
                        RequestVoteRequest{
                            term: state.current_term,
                            candidate_id: id,
                            last_log_index: state.logs.len() as u32,
                            last_log_term: state.logs.last().map(|l| l.term).unwrap_or_else(|| 0)
                        }
                    );
                    request.metadata_mut().insert("grpc-timeout", "1000m".parse().unwrap());

                    // TODO: make concurrent requests to all the other nodes
                    match client.request_vote(request).await {
                        Ok(response) => {
                            if response.get_ref().vote_granted {
                                votes += 1
                            }
                        },
                        Err(_) => {}
                    };
                    if votes >= cfg.majority_vote_count() {
                        // we got the majority of votes, so we are the leader
                        state.mode = Mode::Leader;
                        // clearing volatile state on election (could be some old entries on a re-election)
                        state.reset_leader_state();
                    } else {
                        // if we did not get enough votes
                        // then either it is a split votes scenario
                        // or some other node has become the leader by now
                        // in either case it makes sense to become a follower
                        // for the split votes scenario, on election timeout
                        // this node will retry again
                        state.mode = Mode::Follower;
                    }
                    // TODO prettify logging with a prefix based logger that shows the node-id always
                    println!("election complete, mode: {:?}, node: {}, votes: {}, majority: {}", state.mode, id, votes, cfg.majority_vote_count())
                }
            },
            Mode::Leader => {
                if clock.since(state.last_heartbeat_sent_millis) >= HEARTBEAT_TIMEOUT.as_millis() {
                    for node in &cfg.nodes {
                        let mut request = Request::new(
                            AppendEntriesRequest{
                                term: state.current_term,
                                leader_id: id,
                                prev_log_index: state.logs.len() as u32,
                                prev_log_term: state.logs.last().map(|l| l.term).unwrap_or_else(|| 0),
                                leader_commit: state.commit_index,
                                entries: Vec::new(),
                            }
                        );
                        request.metadata_mut().insert("grpc-timeout", "1000m".parse().unwrap());

                        if node.id == id {
                            continue;
                        }
                        let client = clients.get_mut(&node.id).unwrap();
                        _ = client.append_entries(request).await?.into_inner();
                    }
                    state.last_heartbeat_sent_millis = clock.now();
                }

                let mut ctr = 0;
                let vec = state.next_index.clone();
                for nni in &vec {
                    ctr += 1;
                    if ctr == id {
                        continue;
                    }
                    let len = state.logs.len() as u32;
                    if len >= *nni {
                        let num = len - *nni + 1;
                        let mut entries = vec![String::from(""); num as usize];
                        for i in 0..num {
                            let j = (*nni - 1 + i) as usize;
                            entries[i as usize] = state.logs[j].content.to_string();
                        }
                        let mut request = Request::new(
                            AppendEntriesRequest{
                                term: state.current_term,
                                leader_id: id,
                                prev_log_index: state.logs.len() as u32,
                                prev_log_term: state.logs.last().map(|l| l.term).unwrap_or_else(|| 0),
                                leader_commit: state.commit_index,
                                entries: entries,
                            }
                        );
                        request.metadata_mut().insert("grpc-timeout", "1000m".parse().unwrap());
                        let client = clients.get_mut(&(ctr - 1)).unwrap();
                        let response = client.append_entries(request).await?.into_inner();
                        let idx = (ctr-1) as usize;
                        if response.success {
                            state.next_index[idx] = len + 1;
                            state.match_index[idx] = len;
                        } else {
                            // retry to send this in the next iteration
                            // this could arise because of log inconsistency
                            state.next_index[idx] -= 1;
                        }
                    }
                }

                let new_commit_index = state.derive_commit_index(&cfg);
                if new_commit_index > state.commit_index {
                    state.commit_index = new_commit_index;
                }
            }
        }
        state.persist();
        // releases the lock
        mem::drop(state);
        thread::sleep(MAIN_LOOP_DELAY);
    }

    // server.await?;
    // Ok(())
}
