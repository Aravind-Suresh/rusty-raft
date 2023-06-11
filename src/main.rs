mod state;
use state::State;
use tonic::{transport::Server, Request, Response, Status};
use raft::{AppendEntriesRequest, AppendEntriesResponse, raft_participant_server::{RaftParticipant, RaftParticipantServer}};

mod raft {
    tonic::include_proto!("raft");
}

#[derive(Default)]
pub struct RaftParticipantImpl {}

#[tonic::async_trait]
impl RaftParticipant for RaftParticipantImpl {
    async fn append_entries(&self, request:Request<AppendEntriesRequest>)->Result<Response<AppendEntriesResponse>,Status>{
        Ok(Response::new(AppendEntriesResponse{
             message:format!("hello {}",request.get_ref().name),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = State::restore();
    state.current_term += 1;
    state.persist();
    let addr = "[::1]:50051".parse().unwrap();
    let raft = RaftParticipantImpl::default();
    println!("Server listening on {}", addr);
    Server::builder()
        .add_service(RaftParticipantServer::new(raft))
        .serve(addr)
        .await?;
    Ok(())
}
