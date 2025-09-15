use actix_web::{web, App, HttpServer, HttpResponse, Error, HttpRequest};
use actix_web_actors::ws;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use actix_cors::Cors;
use serde_json;
use uuid::Uuid;
use log::{info, warn};
use actix::*;

// Types pour la signalisation P2P
type ClientId = String;
type RoomId = String;
type Clients = Arc<Mutex<HashMap<ClientId, Addr<SignalingSession>>>>;
type Rooms = Arc<Mutex<HashMap<RoomId, HashSet<ClientId>>>>;

// Messages de signalisation pour WebRTC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    pub action: String,           // "offer", "answer", "ice-candidate", "join", "leave"
    pub room_id: Option<String>,  // ID de la salle de consultation
    pub target: Option<String>,   // Client cible 
    pub data: serde_json::Value,  // Données du signal 
    pub sender: String,           // ID de l'expéditeur
}

// Actor pour la session de signalisation
#[derive(Message)]
#[rtype(result = "()")]
struct TextMessage(String);

// Session WebSocket pour la signalisation
struct SignalingSession {
    id: ClientId,
    room_id: Option<RoomId>,
    clients: Clients,
    rooms: Rooms,
}

impl SignalingSession {
    fn new(clients: Clients, rooms: Rooms) -> Self {
        Self {
       id: Uuid::new_v4().to_string(),
      room_id: None,
     clients,
     rooms,
        }
    }
}

impl Actor for SignalingSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
    info!("Nouvelle session de signalisation: {}", self.id);
        
        // Ajouter le client à la liste globale
    let addr = ctx.address();
    self.clients.lock().unwrap().insert(self.id.clone(), addr);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Session terminée: {}", self.id);
        
        // Retirer le client de toutes les salles
        if let Some(room_id) = self.room_id.take() {
            let mut rooms = self.rooms.lock().unwrap();
            if let Some(clients) = rooms.get_mut(&room_id) {
                clients.remove(&self.id);
                
                // Informer les autres clients du départ
                let leave_msg = serde_json::json!({
                    "action": "user-left",
                    "room_id": &room_id,
                    "client_id": self.id
                }).to_string();
                
                self.broadcast_to_room(&leave_msg, true);
                
                if clients.is_empty() {
                    rooms.remove(&room_id);
                }
            }
            info!("Client {} a quitté la salle {}", self.id, room_id);
        }
        
        // Retirer le client de la liste globale
        self.clients.lock().unwrap().remove(&self.id);
    }
}
impl Handler<TextMessage> for SignalingSession {
    type Result = ();

    fn handle(&mut self, msg: TextMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SignalingSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                let text_str = text.to_string();
                match serde_json::from_str::<SignalMessage>(&text_str) {
                    Ok(signal_msg) => self.handle_signal_message(signal_msg, ctx),
                    Err(e) => {
                        warn!("Message JSON invalide: {}", e);
                        let error_msg = serde_json::json!({
                  "error": "Invalid JSON format",
                  "details": e.to_string()
                        });
                        ctx.text(error_msg.to_string());
                    }
                }
            }
            Ok(ws::Message::Ping(data)) => ctx.pong(&data),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl SignalingSession {
    fn handle_signal_message(&mut self, msg: SignalMessage, ctx: &mut <Self as Actor>::Context) {
        match msg.action.as_str() {
            "join" => {
                if let Some(room_id) = &msg.room_id {
                    self.join_room(room_id.clone());
                    
                    // Répondre au client qu'il a rejoint la salle
                    let join_response = serde_json::json!({
                        "action": "joined",
                        "room_id": room_id,
                        "client_id": self.id,
                        "success": true
                    });
                    ctx.text(join_response.to_string());
                    
                    // Informer les autres clients dans la salle
                    let user_joined_msg = serde_json::json!({
                        "action": "user-joined",
                        "room_id": room_id,
                        "client_id": msg.sender
                    }).to_string();
                    
                    self.broadcast_to_room(&user_joined_msg, false);
                }
            }
            "leave" => {
                self.leave_room();
                
                // Confirmer la sortie
                let leave_response = serde_json::json!({
                    "action": "left",
                    "client_id": self.id,
                    "success": true
                });
                ctx.text(leave_response.to_string());
            }
            "offer" | "answer" | "ice-candidate" => {
                self.relay_message(msg);
            }
            _ => {
                warn!("Action non reconnue: {}", msg.action);
                let error_msg = serde_json::json!({
                    "error": "Unknown action",
                    "action": msg.action
                });
                ctx.text(error_msg.to_string());
            }
        }
    }

    fn join_room(&mut self, room_id: String) {
        let mut rooms = self.rooms.lock().unwrap();
        rooms.entry(room_id.clone())
            .or_insert_with(HashSet::new)
            .insert(self.id.clone());
        
        self.room_id = Some(room_id.clone());
        info!("Client {} a rejoint la salle {}", self.id, room_id);
    }

    fn leave_room(&mut self) {
        if let Some(room_id) = self.room_id.take() {
            let mut rooms = self.rooms.lock().unwrap();
            if let Some(clients) = rooms.get_mut(&room_id) {
                clients.remove(&self.id);
                
                if clients.is_empty() {
                    rooms.remove(&room_id);
                }
            }
            info!("Client {} a quitté la salle {}", self.id, room_id);
        }
    }

    fn relay_message(&self, msg: SignalMessage) {
        if let Ok(msg_json) = serde_json::to_string(&msg) {
            if let Some(target_id) = &msg.target {
                // Message direct à un client spécifique
                let clients = self.clients.lock().unwrap();
                if let Some(target_addr) = clients.get(target_id) {
                    target_addr.do_send(TextMessage(msg_json));
                }
            } else {
                // Diffusion à tous les clients de la salle
                if let Some(room_id) = &self.room_id {
                    self.broadcast_to_room(&msg_json, false);
                }
            }
        }
    }

    fn broadcast_to_room(&self, message: &str, include_self: bool) {
        if let Some(room_id) = &self.room_id {
            let rooms = self.rooms.lock().unwrap();
            let clients = self.clients.lock().unwrap();
            
            if let Some(room_clients) = rooms.get(room_id) {
                for client_id in room_clients {
                    if include_self || client_id != &self.id {
                        if let Some(client_addr) = clients.get(client_id) {
                            client_addr.do_send(TextMessage(message.to_string()));
                        }
                    }
                }
            }
        }
    }
}

// Route WebSocket
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    clients: web::Data<Clients>,
    rooms: web::Data<Rooms>,
) -> Result<HttpResponse, Error> {
    info!(" Nouvelle connexion WebSocket attempt");
    let result = ws::start(
        SignalingSession::new(clients.get_ref().clone(), rooms.get_ref().clone()),
        &req,
        stream,
    );
    info!("Connexion WebSocket result: {:?}", result.is_ok());
    result
}


// Route to see if server is working
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "video-signaling-server",
        "version": "1.0.0"
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));

    info!(" Serveur de signalisation WebRTC démarré sur 127.0.0.1:8080");
    
    HttpServer::new(move || {
        let cors = Cors::default()
        .allow_any_origin()
        .allow_any_method()
        .allow_any_header()
        .supports_credentials();

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(clients.clone()))
            .app_data(web::Data::new(rooms.clone()))
            .route("/health", web::get().to(health_check))
            .route("/ws", web::get().to(ws_route))
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}