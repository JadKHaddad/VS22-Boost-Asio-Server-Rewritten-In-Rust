use futures_util::{future, pin_mut, StreamExt};
use shared::Direction;
use shared::Message as SharedMessage;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() {
    let url = url::Url::parse("ws://localhost:3000/").unwrap();
    let (tx, rx) = futures_channel::mpsc::unbounded();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    let (write, read) = ws_stream.split();
    let ws_out = rx.map(Ok).forward(write);
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
                                    if tx.unbounded_send(Message::Text(msg)).is_err() {
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
    pin_mut!(ws_out, ws_in);
    future::select(ws_out, ws_in).await;
}
