mod state;
mod clock;

use state::{State, LogEntry};
use tonic::{transport::Server, Request, Response, Status};
use raft::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse, raft_participant_server::{RaftParticipant, RaftParticipantServer}};
use std::sync::{RwLock, Arc};
use clock::Clock;
use std::{cmp, thread, time};
use std::mem;

const MAIN_LOOP_DELAY: time::Duration = time::Duration::from_secs(10);

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
    let raft = RaftParticipantImpl{
        state: RwLock::new(State::restore()),
        clock: clock,
    };

    let addr = "[::1]:50051".parse().unwrap();
    println!("Server listening on {}", addr);
    let _server = Server::builder()
        .add_service(RaftParticipantServer::new(raft))
        .serve(addr);

    loop {
        println!("new run of main loop, started at = {}", clock.now());
        thread::sleep(MAIN_LOOP_DELAY)
    }

    // server.await?;
    // Ok(())
}
