use futures_util::{SinkExt, StreamExt};
use poem::{
    get, handler,
    listener::TcpListener,
    web::{
        websocket::{Message, WebSocket},
        Data,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use server::*;
use std::sync::Arc;
use tokio::sync::mpsc::channel;

#[handler]
fn ws(ws: WebSocket, game: Data<&Arc<Game>>) -> impl IntoResponse {
    let id = game.create_id();
    let position = game.create_random_position();
    let (tx, mut rx) = channel::<String>(100);
    let color = game.create_random_color();
    let client = Client::new(id, position, tx, color);

    let mut receiver_client = client.clone();
    let sender_client = client.clone();

    game.add_client(client);

    let game_sender = game.clone();
    let game_receiver = game.clone();

    if game.start_game() {
        let game_c = game.clone();
        tokio::spawn(async move {
            game_c.run().await;
        });
    }

    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(text) = msg {
                    game_receiver.on_new_message(&mut receiver_client, text);
                }
            }
            game_receiver.remove_client(&receiver_client);
        });

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    game_sender.remove_client(&sender_client);
                    break;
                }
            }
        });
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let game = Arc::new(Game::new(5, 5, 1));
    game.display_field_once();
    let app = Route::new().at("/", get(ws.data(game)));

    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
