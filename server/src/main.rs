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
    let id = game.get_total_clients();
    let position = game.create_random_position();
    let (tx, mut rx) = channel::<String>(100);
    let color = game.create_random_color();
    let client = Client::new(id, position, tx, color);

    let mut receiver_client = client.clone();

    game.add_client(client);

    let game_sender = game.clone();
    let game_receiver = game.clone();

    if id >= game.get_max_clients() - 1 {
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
        });

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    game_sender.remove_client(id);
                    break;
                }
            }
        });
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let game = Arc::new(Game::new(4, 4, 3));

    let app = Route::new().at("/", get(ws.data(game)));

    println!("Waiting for clients...");
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
