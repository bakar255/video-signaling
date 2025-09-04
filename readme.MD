## WebRTC Signaling Server for Teleconsultation

A high-performance, real-time WebRTC signaling server built with Rust and Actix, designed specifically for peer-to-peer teleconsultation applications. This server handles the signaling layer required for establishing direct video/audio connections between healthcare practitioners and patients, ensuring secure and efficient communication without intermediate servers relaying media streams. 

For the context this projet has been done for a wellness platform SaaS.

The server acts as the signaling backbone for WebRTC connections while maintaining complete privacy—all media streams flow directly between participants, ensuring patient confidentiality and compliance with healthcare data protection regulations.

## Key Features

Secure Session Management: Creates isolated consultation rooms with unique IDs for each session

Dynamic Room Handling: Automatically manages room creation and cleanup when sessions end

- Real-Time WebSocket Communication

Bi-directional Communication: Instant message delivery between server and clients
Scalable Architecture: Handles multiple concurrent consultations efficiently