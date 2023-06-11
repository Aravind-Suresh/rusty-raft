mod state;
use state::{State, LogEntry};
use tonic::{transport::Server, Request, Response, Status};
use raft::{AppendEntriesRequest, AppendEntriesResponse, raft_participant_server::{RaftParticipant, RaftParticipantServer}};
use std::sync::RwLock;
use std::cmp;

mod raft {
    tonic::include_proto!("raft");
}

pub struct RaftParticipantImpl {
    state: RwLock<State>,
}

#[tonic::async_trait]
impl RaftParticipant for RaftParticipantImpl {
    async fn append_entries(&self, request:Request<AppendEntriesRequest>)->Result<Response<AppendEntriesResponse>,Status>{
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
            state.logs.push(LogEntry {
                content: entry.to_string(),
                term: current_term
            })
        }
        if request.leader_commit > state.commit_index {
            state.commit_index = cmp::min(request.leader_commit, state.logs.len() as u32)
        }
        Ok(Response::new(AppendEntriesResponse{
            term: state.current_term,
            success: true,
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raft = RaftParticipantImpl{
        state: RwLock::new(State::restore()),
    };

    let addr = "[::1]:50051".parse().unwrap();
    println!("Server listening on {}", addr);
    Server::builder()
        .add_service(RaftParticipantServer::new(raft))
        .serve(addr)
        .await?;
    Ok(())
}
