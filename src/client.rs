use raft::{AppendEntriesRequest, raft_participant_client::{RaftParticipantClient}};

mod raft {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RaftParticipantClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(
        AppendEntriesRequest {
           term: 1,
           leader_id: 0,
           prev_log_index: 0,
           prev_log_term: 0,
           leader_commit: 0,
           entries: Vec::new(),
        },
    );
    let response = client.append_entries(request).await?.into_inner();
    println!("RESPONSE={:?}", response);
    Ok(())
}
