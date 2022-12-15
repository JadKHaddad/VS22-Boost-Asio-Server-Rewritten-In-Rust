use shared::Direction;
use shared::Message as SharedMessage;
use websocket::ClientBuilder;
use websocket::Message;
use websocket::OwnedMessage;
fn main() {
    let client = ClientBuilder::new("ws://localhost:3000/")
        .unwrap()
        .connect_insecure()
        .unwrap();

    let (mut receiver, mut sender) = client.split().unwrap();

    for message in receiver.incoming_messages() {
        match message {
            Ok(message) => {
                match message {
                    OwnedMessage::Text(msg) => {
                        if let Ok(msg) = SharedMessage::from_json(&msg) {
                            match msg {
                                SharedMessage::Position(position) => {
                                    println!("Position: {}, {}", position.x, position.y);
                                    //create a random direction
                                    let direction = Direction::random();
                                    let msg = SharedMessage::new_move(direction).to_json().unwrap();
                                    let m = Message::text(msg);
                                    match sender.send_message(&m) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error: {:?}", e);
                                            break;
                                        }
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
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
        }
    }
}
