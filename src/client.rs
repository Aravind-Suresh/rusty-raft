use raft::{LogRequest, LogStatus, raft_participant_client::{RaftParticipantClient}};
use std::{thread, time::Duration};

mod raft {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RaftParticipantClient::connect("http://[::1]:50052").await?;
    let mut ctr = 0;
    while ctr < 10 {
        let request = tonic::Request::new(
            LogRequest {
                entries: Vec::from(["message ".to_owned() + &ctr.to_string()]),
            },
        );
        let response = client.log(request).await?.into_inner();
        println!("RESPONSE={:?}", response);
        if response.status == LogStatus::Acknowledged.into() {
            ctr += 1;
        } else if response.status == LogStatus::Redirect.into() && response.redirect_url != "" {
            client = RaftParticipantClient::connect(response.redirect_url).await?;
        }
        thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
