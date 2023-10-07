#[allow(dead_code)]
mod constants;
mod messages;
mod packing;
mod utils;

use crate::messages::*;
use crate::packing::UnpackFromBytes;

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::thread;

use input_macro::input;

/// Prints the messages received from peers.
// Like the mainloop but for peer instead of server messages
fn listener_logging(l: TcpListener) {
    for stream in l.incoming() {
        let mut stream = stream.unwrap();
        println!("Connected to peer at {}", stream.local_addr().unwrap());

        let mut length: [u8; 4] = [0, 0, 0, 0];
        let _ = stream.read_exact(&mut length);
        let length = u32::from_le_bytes(length);
        let mut bytes: Vec<u8> = vec![0; length as usize];
        let _ = stream.read_exact(&mut bytes);
        let code = MessageType::PeerInit(<u8>::unpack_from_bytes(&mut bytes));
        println!("Received code from peer: {code:?}");
        match code {
            MessageType::PeerInit(0) => {
                println!(
                    "from peer: {:?}",
                    <PierceFireWall>::unpack_from_bytes(&mut bytes)
                );
            }
            MessageType::PeerInit(1) => {
                println!("from peer: {:?}", <PeerInit>::unpack_from_bytes(&mut bytes));
            }
            _ => {
                println!("received unknown code: {code:?}")
            }
        }

        loop {
            let mut length: [u8; 4] = [0, 0, 0, 0];
            let _ = stream.read_exact(&mut length);
            let length = u32::from_le_bytes(length);
            let mut bytes: Vec<u8> = vec![0; length as usize];
            let _ = stream.read_exact(&mut bytes);
            let code = MessageType::Peer(<u32>::unpack_from_bytes(&mut bytes));
            println!("Received code from peer: {code:?}");

            match code {
                MessageType::Peer(4) => {
                    println!(
                        "from peer: {:?}",
                        GetSharedFileList::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(5) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(9) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(15) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(16) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(36) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(37) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(40) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(41) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(43) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(44) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(46) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(50) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(51) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                MessageType::Peer(52) => {
                    println!(
                        "from peer: {:?}",
                        SharedFileListResponse::from_stream(&mut bytes)
                    );
                }
                _ => {
                    println!("Received unknown message: {code:?}");
                    // Skip the unknown message
                    let _ = stream.read_exact(&mut vec![0u8; length as usize]);
                }
            }
        }
    }
}

fn main() {
    let username = input!("Username: ");
    let password = input!("Password: ");
    let login_info = _SendLogin::new(username.into(), password.into());
    let listener = TcpListener::bind("0.0.0.0:0").unwrap();
    let my_port: u32 = listener.local_addr().unwrap().port().into();
    let set_port = SetWaitPort {
        port: my_port, // 2242,
        use_obfuscation: false, // Leave this until the actual logic for it is implemented
        obfuscated_port: 0,
    };
    thread::spawn(|| listener_logging(listener));

    if let Ok(mut stream) = TcpStream::connect("server.slsknet.org:2242") {
        println!("Connected to the server!");
        let _ = Login::to_stream(&stream, login_info);
        let _ = SetWaitPort::to_stream(&stream, set_port);
        loop {
            let mut length: [u8; 4] = [0, 0, 0, 0];
            let _ = stream.read_exact(&mut length);
            let length = u32::from_le_bytes(length);
            let mut bytes: Vec<u8> = vec![0; length as usize];
            let _ = stream.read_exact(&mut bytes);
            let code = MessageType::Server(<u32>::unpack_from_bytes(&mut bytes));
            println!("Received code: {code:?}");
            match code {
                MessageType::Server(1) => {
                    let response = Login::from_stream(&mut bytes);
                    println!("{:#?}", response);
                    if !response.success {
                        let reason = match response.failure_reason {
                            Some(reason) => reason,
                            None => "reason unknown".to_string(),
                        };
                        println!("Failed to login: {reason}");
                        break;
                    } else {
                        println!("Logged in succesfully.")
                    }
                }
                MessageType::Server(3) => {
                    println!("{:#?}", GetPeerAddress::from_stream(&mut bytes));
                }
                MessageType::Server(5) => {
                    println!("{:#?}", WatchUser::from_stream(&mut bytes));
                }
                MessageType::Server(7) => {
                    println!("{:#?}", GetUserStatus::from_stream(&mut bytes));
                }
                MessageType::Server(13) => {
                    println!("{:#?}", SayChatroom::from_stream(&mut bytes));
                }
                MessageType::Server(14) => {
                    println!("{:#?}", JoinRoom::from_stream(&mut bytes));
                }
                MessageType::Server(15) => {
                    println!("{:#?}", LeaveRoom::from_stream(&mut bytes));
                }
                MessageType::Server(16) => {
                    println!("{:#?}", UserJoinedRoom::from_stream(&mut bytes));
                }
                MessageType::Server(17) => {
                    println!("{:#?}", UserLefRoom::from_stream(&mut bytes));
                }
                MessageType::Server(18) => {
                    // Ideally, this shouldn't happen if we receive the PeerInit message
                    // While still testing stuff out it's not the end of the world
                    // TODO: Handle peer connections appropriately depending on if we have an open port
                    let connect_req = ConnectToPeer::from_stream(&mut bytes);
                    println!("{:#?}", connect_req);
                    let token = connect_req.firewall_token;
                    let fw = PierceFireWall { token };
                    let peer =
                        TcpStream::connect((connect_req.ip, connect_req.port as u16)).unwrap();
                    print!(" Attempting to connect indirectly to {peer:?}");
                    let _ = PierceFireWall::to_stream(&peer, fw);
                }
                MessageType::Server(22) => {
                    println!("{:#?}", MessageUser::from_stream(&mut bytes));
                }
                MessageType::Server(26) => {
                    println!("{:#?}", FileSearch::from_stream(&mut bytes));
                }
                MessageType::Server(36) => {
                    println!("{:#?}", GetUserStats::from_stream(&mut bytes));
                }
                MessageType::Server(41) => {
                    println!("{:#?}", Relogged::from_stream(&mut bytes));
                    println!("This town's not big enough for the both of us!");
                    println!("(Somebody else logged into your account)");
                    break;
                }
                MessageType::Server(64) => {
                    println!("{:#?}", RoomList::from_stream(&mut bytes));
                }
                MessageType::Server(66) => {
                    println!("{:#?}", AdminMessage::from_stream(&mut bytes));
                }
                MessageType::Server(69) => {
                    println!("{:#?}", PrivilegedUsers::from_stream(&mut bytes));
                }
                MessageType::Server(83) => {
                    println!("{:#?}", ParentMinSpeed::from_stream(&mut bytes));
                }
                MessageType::Server(84) => {
                    println!("{:#?}", ParentSpeedRatio::from_stream(&mut bytes));
                }
                MessageType::Server(92) => {
                    println!("{:#?}", CheckPrivileges::from_stream(&mut bytes));
                }
                MessageType::Server(93) => {
                    println!("{:#?}", EmbeddedMessage::from_stream(&mut bytes));
                }
                MessageType::Server(102) => {
                    println!("{:#?}", PossibleParents::from_stream(&mut bytes));
                }
                MessageType::Server(104) => {
                    println!("{:#?}", WishListInterval::from_stream(&mut bytes));
                }
                MessageType::Server(113) => {
                    println!("{:#?}", RoomTickerState::from_stream(&mut bytes));
                }
                MessageType::Server(114) => {
                    println!("{:#?}", RoomTickerAdd::from_stream(&mut bytes));
                }
                MessageType::Server(115) => {
                    println!("{:#?}", RoomTickerRemove::from_stream(&mut bytes));
                }
                MessageType::Server(130) => {
                    println!("{:#?}", ResetDistributed::from_stream(&mut bytes));
                }
                MessageType::Server(133) => {
                    println!("{:#?}", PrivateRoomUsers::from_stream(&mut bytes));
                }
                MessageType::Server(134) => {
                    println!("{:#?}", PrivateRoomAddUser::from_stream(&mut bytes));
                }
                MessageType::Server(135) => {
                    println!("{:#?}", PrivateRoomRemoveUser::from_stream(&mut bytes));
                }
                MessageType::Server(139) => {
                    println!("{:#?}", PrivateRoomAdded::from_stream(&mut bytes));
                }
                MessageType::Server(140) => {
                    println!("{:#?}", PrivateRoomRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(141) => {
                    println!("{:#?}", PrivateRoomToggle::from_stream(&mut bytes));
                }
                MessageType::Server(142) => {
                    println!("{:#?}", ChangePassword::from_stream(&mut bytes));
                }
                MessageType::Server(143) => {
                    println!("{:#?}", PrivateRoomAddOperator::from_stream(&mut bytes));
                }
                MessageType::Server(144) => {
                    println!("{:#?}", PrivateRoomRemoveOperator::from_stream(&mut bytes));
                }
                MessageType::Server(145) => {
                    println!("{:#?}", PrivateRoomOperatorAdded::from_stream(&mut bytes));
                }
                MessageType::Server(146) => {
                    println!("{:#?}", PrivateRoomOperatorRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(148) => {
                    println!("{:#?}", PrivateRoomOwned::from_stream(&mut bytes));
                }
                MessageType::Server(1001) => {
                    println!("{:#?}", CantConnectToPeer::from_stream(&mut bytes));
                }
                MessageType::Server(1003) => {
                    println!("{:#?}", CantConnectToRoom::from_stream(&mut bytes));
                }
                _ => {
                    println!("Received unknown message: {code:?}");
                    // Skip the unknown message
                    let _ = stream.read_exact(&mut vec![0u8; length as usize]);
                }
            }
        }
    } else {
        println!("Couldn't connect to server...");
    }
}
