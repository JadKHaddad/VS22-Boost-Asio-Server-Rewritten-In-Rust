use futures_util::{SinkExt, StreamExt};
use shared::{Direction, Message as SharedMessage};
use std::process::exit;
use tokio::{signal, sync::mpsc::unbounded_channel};
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

    let tx_signal = tx.clone();
    let signal = tokio::spawn(async move {
        let mut sig_count = 0;
        loop {
            signal::ctrl_c().await.unwrap();
            sig_count += 1;
            if sig_count > 1 {
                exit(1);
            }
            // create disconnect message
            let msg = SharedMessage::new_disconnect().to_json().unwrap();
            let _ = tx_signal.send(Message::Text(msg));
            println!("Waiting for server to respond. Press ctr+c again to exit");
        }
    });

    let ws_in = {
        read.for_each(|message| async {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        let msg = SharedMessage::from_json(&text).unwrap();
                        match msg {
                            SharedMessage::Position(position) => {
                                println!("Position: {}, {}", position.x, position.y);
                                let direction = Direction::random();
                                let msg = SharedMessage::new_move(direction).to_json().unwrap();
                                let _ = tx.send(Message::Text(msg));
                            }
                            SharedMessage::Score(score) => {
                                println!("Score: {}", score);
                                exit(0);
                            }
                            _ => {}
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
        _ = signal => {}
    }
}
