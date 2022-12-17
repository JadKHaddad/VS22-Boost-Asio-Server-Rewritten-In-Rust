use futures_util::{SinkExt, StreamExt};
use shared::{Direction, Message as SharedMessage};
use tokio::sync::mpsc::unbounded_channel;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() {
    let url = url::Url::parse("ws://localhost:3000/").unwrap();
    let (tx, mut rx) = unbounded_channel::<Message>();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    let (mut write, read) = ws_stream.split();

    let ws_out = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let ws_in = {
        read.for_each(|message| async {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        if let Ok(msg) = SharedMessage::from_json(&text) {
                            match msg {
                                SharedMessage::Position(position) => {
                                    println!("Position: {}, {}", position.x, position.y);
                                    let direction = Direction::random();
                                    let msg = SharedMessage::new_move(direction).to_json().unwrap();
                                    if tx.send(Message::Text(msg)).is_err() {
                                        println!("Error sending message");
                                    }
                                }
                                SharedMessage::Score(score) => {
                                    println!("Score: {:?}", score);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
    };
    tokio::select! {
        _ = ws_in => {}
        _ = ws_out => {}
    }
}
