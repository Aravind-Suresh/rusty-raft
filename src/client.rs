use raft::{AppendEntriesRequest, AppendEntriesResponse, raft_participant_client::{RaftParticipantClient}};

mod raft {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RaftParticipantClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(
        AppendEntriesRequest {
           name:String::from("world")
        },
    );
    let response = client.append_entries(request).await?.into_inner();
    println!("RESPONSE={:?}", response);
    Ok(())
}
