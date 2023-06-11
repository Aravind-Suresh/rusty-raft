mod state;
mod clock;
mod config;

use state::{State, LogEntry};
use tonic::{transport::Server, Request, Response, Status};
use raft::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse, raft_participant_server::{RaftParticipant, RaftParticipantServer}};
use std::sync::{RwLock, Arc};
use clock::Clock;
use std::{cmp, thread, time};
use std::mem;
use config::{Config, Node};
use std::collections::HashMap;

const MAIN_LOOP_DELAY: time::Duration = time::Duration::from_secs(10);
const ELECTION_TIMEOUT: time::Duration = time::Duration::from_millis(200);

mod raft {
    tonic::include_proto!("raft");
}

pub struct RaftParticipantImpl<C: Clock> {
    state: RwLock<State>,
    clock: C,
}

#[tonic::async_trait]
impl<C: Clock + 'static> RaftParticipant for RaftParticipantImpl<C> {
    async fn append_entries(
        &self, 
        request: Request<AppendEntriesRequest>
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let mut state = self.state.write().unwrap();
        let request = request.get_ref();
        let response_false = Ok(Response::new(AppendEntriesResponse{
            term: state.current_term,
            success: false,
        }));
        if request.term < state.current_term {
            return response_false
        }
        // TODO: check out of bounds
        if state.logs.get(request.prev_log_index as usize).unwrap().term != request.prev_log_term {
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
        state.last_heartbeat_at_millis = self.clock.now();
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clock = clock::SystemClock{};
    // TODO: move this to a config file and read it
    // TODO: add config validation checks
    let cfg = Config{
        nodes: [
            Node{
                id: 1,
                addr: "[::1]:50051",
            },
            Node {
                id: 2,
                addr: "[::1]:50052",
            }
        ]
    }
    let mut clients: HashMap<u32, &RaftParticipantClient> = HashMap::new();
    // TODO: this should be read from sys args
    let id = 1;
    let raft = RaftParticipantImpl{
        state: RwLock::new(State::restore(id)),
        clock: clock,
    };

    let addr = "[::1]:50051".parse().unwrap();
    println!("Server listening on {}", addr);
    let _server = Server::builder()
        .add_service(RaftParticipantServer::new(raft))
        .serve(addr);

    loop {
        println!("new run of main loop, started at = {}", clock.now());
        // TODO: do optimistic locking here
        let state = raft.state.write().unwrap()
        match state.mode {
            State::Follower => {
                if clock.since(state.last_heartbeat_at_millis) >= ELECTION_TIMEOUT {
                    state.mode = Mode::Candidate
                }
            },
            State::Candidate => {
                // conduct an election here
                state.current_term += 1
                state.voted_for = state.id
                state.last_heartbeat_at_millis = clock.now()
                let mut votes = 0
                for node in cfg.nodes {
                    if node.id == id {
                        continue
                    }
                    clients.entry(node.id).or_insert_with(|| &RaftParticipantClient::connect(node.addr).await?);
                    let client = clients.get(node.id);
                    let request = Request::new(
                        RequestVoteRequest{
                            term: state.current_term,
                            candidate_id: id,
                            last_log_index: state.logs.len(),
                            last_log_term: state.logs.top().unwrap_or_else(0)
                        }
                    );
                    votes += match client.request_vote(request).await() {
                        Some(response) => {
                            if response.vote_granted {
                                return 1
                            }
                            return 0
                        },
                        _ => {
                            return 0
                        }
                    }
                    if votes > cfg.majority_vote_count() {
                        // we got the majority of votes, so we are the leader
                        state.mode = Mode::Leader
                    } else {
                        // if we did not get enough votes
                        // then either it is a split votes scenario
                        // or some other node has become the leader by now
                        // in either case it makes sense to become a follower
                        // for the split votes scenario, on election timeout
                        // this node will retry again
                        state.mode = Mode::Follower
                    }
                }
            },
        }
        // releases the lock
        mem::drop(state)
        thread::sleep(MAIN_LOOP_DELAY)
    }

    // server.await?;
    // Ok(())
}
