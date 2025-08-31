use actix_web::{web, App, HttpServer, HttpResponse, Error, HttpRequest};
use actix_web_actors::ws;
use actix_web_actors::ws::Message;
use uuid::Uuid;
use log::info;

type Tx = Arc<UnboundedSender<String>>;
type Rooms = Arc<Mutex<HashMap<String,Vec<Tx>>>;


// Struct represent who clients-sessions
struct WsSession {
    id: String,
}

// Route upgrade from HTTP to WebSocket 
async fn ws_route(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
     ws::start(WsSession::new(), &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
          .route("/", web::get().to(|| async {HttpResponse::Ok().body("Hello Actix ! ") }))
          .route("/ws", web::get().to(ws_route))

    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}


impl StreamHandler<Result<Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(message::Text(text)) => {
                ctx.text(format! )
            }
        }
    }
}