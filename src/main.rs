#[macro_use]
mod macros;
#[allow(dead_code)]
mod constants;
mod events;
mod gui;
mod messages;
mod packing;
mod styles;
mod utils;

use crate::events::SLSKEvents;
use crate::messages::*;
use crate::packing::UnpackFromBytes;
use crate::utils::keepalive_add_retries;

use smol::{block_on, Timer};
use socket2::{SockRef, TcpKeepalive};
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{broadcast::channel, Mutex};
use tokio::task::JoinError;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const QUEUE_SIZE: usize = 1000;

    let (write_queue, read_queue) = channel::<SLSKEvents>(QUEUE_SIZE);
    let read_queue_copy = read_queue.resubscribe();
    let write_queue_copy = write_queue.clone();

    thread::spawn(move || match gui::main(write_queue_copy, read_queue_copy) {
        Ok(()) => {
            return;
        }
        Err(e) => panic!("{e}"),
    });
    let mut login_timeout: u64 = 15;

    let mut temp_receiver = read_queue.resubscribe();

    loop {
        let should_quit = async {
            let mut quit = false;
            while !quit {
                let event = temp_receiver.recv().await;
                quit = match event {
                    Ok(event) => match event {
                        SLSKEvents::Quit => true,
                        _ => false,
                    },
                    Err(_) => false,
                };
            }
            quit
        };

        let stream: Result<TcpStream, Error> = tokio::select! {
            // Wait for the connection to complete
            connect_result = TcpStream::connect("server.slsknet.org:2242") => connect_result,
            // Wait for should_quit to complete
            quit = should_quit => {
                if quit {
                    Err(ErrorKind::Other.into())
                } else {unimplemented!()}
            }
        };

        // panic!("Result: {:?}", stream);

        // println!("trying to connect");
        // let stream: Result<TcpStream, Error> = TcpStream::connect("server.slsknet.org:2242").await;
        match stream {
            Ok(stream) => {
                let sock_ref = SockRef::from(&stream);
                // These specific settings are based on nicotine+'s settings
                let mut ka = TcpKeepalive::new()
                    .with_time(Duration::from_secs(10))
                    .with_interval(Duration::from_secs(2));
                ka = keepalive_add_retries(ka);
                sock_ref.set_tcp_keepalive(&ka)?;
                // TODO: Set the number of TCP pings allowed before the connection is assumed to be dead (should be 10)
                // on platforms where it's not supported by socket2 (e.g. Windows)
                // Not 100% sure this is actually possible as it's an OS limitation, but I imagine it could be implemented manually.

                let handle = tokio::spawn(handle_client(
                    stream,
                    write_queue.clone(),
                    read_queue.resubscribe(),
                ));
                match handle.await {
                    Ok(slskexit) => match slskexit {
                        SLSKExitCode::LoginFail => (),
                        _ => {
                            break;
                        }
                    },
                    Err(_) => unimplemented!(),
                };
            }
            Err(e) => {
                let kind = e.kind();

                if kind == ErrorKind::Other {
                    // If we receive SLSKEvent::Quit from the TUI
                    break;
                } else if e.raw_os_error() == Some(11001) {
                    // If "no such host is known" (Not connected to the internet?)
                    continue;
                }
                println!(
                    "stream fail: {e}, sleeping for {login_timeout} seconds. {:?}",
                    e
                );
                Timer::after(Duration::from_secs(login_timeout)).await;
                login_timeout *= 2;
            }
        };
    }
    Ok(())
}

#[derive(Debug)]
enum SLSKExitCode {
    Ok,
    LoginFail,
    JoinError(JoinError),
    OtherLogin,
    IoError(Error),
}

async fn handle_client(
    stream: TcpStream,
    write_queue: Sender<SLSKEvents>,
    mut read_queue: Receiver<SLSKEvents>,
) -> SLSKExitCode {
    let (mut reader, mut writer) = stream.into_split();

    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let my_port: u32 = listener.local_addr().unwrap().port().into();

    let quit = Arc::new(Mutex::new(false));
    let quit_r = Arc::clone(&quit);
    let quit_w = Arc::clone(&quit);

    // Spawn separate tasks for reading and writing
    let read_task = tokio::spawn(async move {
        loop {
            if *quit_r.lock().await {
                return SLSKExitCode::Ok;
            };
            let mut length: [u8; 4] = [0, 0, 0, 0];
            match reader.read_exact(&mut length).await {
                Ok(_) => (),
                Err(e) => return SLSKExitCode::IoError(e),
            }
            let length = u32::from_le_bytes(length);
            let mut bytes: Vec<u8> = vec![0; length as usize];
            let _ = reader.read_exact(&mut bytes).await;
            let code = MessageType::Server(<u32>::unpack_from_bytes(&mut bytes));

            match code {
                MessageType::Server(1) => {
                    let response = Login::from_stream(&mut bytes);
                    let _ = write_queue.send(SLSKEvents::LoginResult {
                        success: response.success,
                        reason: response.failure_reason,
                    });
                    if !response.success {
                        return SLSKExitCode::LoginFail;
                    }
                }
                MessageType::Server(3) => {
                    // println!("{:#?}", GetPeerAddress::from_stream(&mut bytes));
                }
                MessageType::Server(5) => {
                    // println!("{:#?}", WatchUser::from_stream(&mut bytes));
                }
                MessageType::Server(7) => {
                    // println!("{:#?}", GetUserStatus::from_stream(&mut bytes));
                }
                MessageType::Server(13) => {
                    let response = SayChatroom::from_stream(&mut bytes);
                    // println!("{response:#?}");
                    let _ = write_queue.send(SLSKEvents::ChatroomMessage {
                        room: response.room,
                        username: Some(response.username),
                        message: response.message,
                    });
                }
                MessageType::Server(14) => {
                    let response = JoinRoom::from_stream(&mut bytes);
                    let _ = write_queue.send(SLSKEvents::UpdateRoom {
                        room: response.room,
                        stats: response
                            .usernames
                            .into_iter()
                            .zip(response.stats)
                            .collect::<Vec<(String, UserStats)>>(),
                    });
                }
                MessageType::Server(15) => {
                    // println!("{:#?}", LeaveRoom::from_stream(&mut bytes));
                }
                MessageType::Server(16) => {
                    // println!("{:#?}", UserJoinedRoom::from_stream(&mut bytes));
                }
                MessageType::Server(17) => {
                    // println!("{:#?}", UserLefRoom::from_stream(&mut bytes));
                }
                MessageType::Server(18) => {
                    // Ideally, this shouldn't happen if we receive the PeerInit message
                    // While still testing stuff out it's not the end of the world
                    // TODO: Handle peer connections appropriately depending on if we have an open port
                    let connect_req = ConnectToPeer::from_stream(&mut bytes);
                    let token = connect_req.firewall_token;
                    let fw = PierceFireWall { token };
                    let peer = TcpStream::connect((connect_req.ip, connect_req.port as u16)).await;
                    if peer.is_ok() {
                        let _ = PierceFireWall::async_write_to(&mut peer.unwrap(), fw).await;
                    }
                }
                MessageType::Server(22) => {
                    // println!("{:#?}", MessageUser::from_stream(&mut bytes));
                }
                MessageType::Server(26) => {
                    // println!("{:#?}", FileSearch::from_stream(&mut bytes));
                }
                MessageType::Server(36) => {
                    // println!("{:#?}", GetUserStats::from_stream(&mut bytes));
                }
                MessageType::Server(41) => {
                    // println!("{:#?}", Relogged::from_stream(&mut bytes));
                    return SLSKExitCode::OtherLogin;
                }
                MessageType::Server(64) => {
                    let room_list = RoomList::from_stream(&mut bytes);
                    let rooms_and_num_of_users = room_list
                        .rooms
                        .into_iter()
                        .zip(room_list.num_of_users)
                        .collect();
                    let _ = write_queue.send(SLSKEvents::RoomList {
                        rooms_and_num_of_users,
                    });
                }
                MessageType::Server(66) => {
                    // println!("{:#?}", AdminMessage::from_stream(&mut bytes));
                }
                MessageType::Server(69) => {
                    // println!("{:#?}", PrivilegedUsers::from_stream(&mut bytes));
                }
                MessageType::Server(83) => {
                    // println!("{:#?}", ParentMinSpeed::from_stream(&mut bytes));
                }
                MessageType::Server(84) => {
                    // println!("{:#?}", ParentSpeedRatio::from_stream(&mut bytes));
                }
                MessageType::Server(92) => {
                    // println!("{:#?}", CheckPrivileges::from_stream(&mut bytes));
                }
                MessageType::Server(93) => {
                    // println!("{:#?}", EmbeddedMessage::from_stream(&mut bytes));
                }
                MessageType::Server(102) => {
                    // println!("{:#?}", PossibleParents::from_stream(&mut bytes));
                }
                MessageType::Server(104) => {
                    // println!("{:#?}", WishListInterval::from_stream(&mut bytes));
                }
                MessageType::Server(113) => {
                    // println!("{:#?}", RoomTickerState::from_stream(&mut bytes));
                }
                MessageType::Server(114) => {
                    // println!("{:#?}", RoomTickerAdd::from_stream(&mut bytes));
                }
                MessageType::Server(115) => {
                    // println!("{:#?}", RoomTickerRemove::from_stream(&mut bytes));
                }
                MessageType::Server(130) => {
                    // println!("{:#?}", ResetDistributed::from_stream(&mut bytes));
                }
                MessageType::Server(133) => {
                    // println!("{:#?}", PrivateRoomUsers::from_stream(&mut bytes));
                }
                MessageType::Server(134) => {
                    // println!("{:#?}", PrivateRoomAddUser::from_stream(&mut bytes));
                }
                MessageType::Server(135) => {
                    // println!("{:#?}", PrivateRoomRemoveUser::from_stream(&mut bytes));
                }
                MessageType::Server(139) => {
                    // println!("{:#?}", PrivateRoomAdded::from_stream(&mut bytes));
                }
                MessageType::Server(140) => {
                    // println!("{:#?}", PrivateRoomRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(141) => {
                    // println!("{:#?}", PrivateRoomToggle::from_stream(&mut bytes));
                }
                MessageType::Server(142) => {
                    // println!("{:#?}", ChangePassword::from_stream(&mut bytes));
                }
                MessageType::Server(143) => {
                    // println!("{:#?}", PrivateRoomAddOperator::from_stream(&mut bytes));
                }
                MessageType::Server(144) => {
                    // println!("{:#?}", PrivateRoomRemoveOperator::from_stream(&mut bytes));
                }
                MessageType::Server(145) => {
                    // println!("{:#?}", PrivateRoomOperatorAdded::from_stream(&mut bytes));
                }
                MessageType::Server(146) => {
                    // println!("{:#?}", PrivateRoomOperatorRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(148) => {
                    // println!("{:#?}", PrivateRoomOwned::from_stream(&mut bytes));
                }
                MessageType::Server(1001) => {
                    // println!("{:#?}", CantConnectToPeer::from_stream(&mut bytes));
                }
                MessageType::Server(1003) => {
                    // println!("{:#?}", CantConnectToRoom::from_stream(&mut bytes));
                }
                _ => {
                    let _ = reader.read_exact(&mut vec![0u8; length as usize]).await;
                }
            }
        }
    });

    let write_task = tokio::spawn({
        async move {
            loop {
                let event = read_queue.recv().await;
                match event {
                    Ok(event) => match event {
                        SLSKEvents::TryLogin { username, password } => {
                            let login_info = _SendLogin::new(username, password);
                            let _ = block_on(Login::async_write_to(&mut writer, login_info).await);
                        }
                        SLSKEvents::Quit => {
                            *quit_w.lock().await = true;
                            let _ = writer.shutdown();
                            return SLSKExitCode::Ok;
                        }
                        SLSKEvents::LoginResult { success, .. } => {
                            if success {
                                let _ = block_on(
                                    SetWaitPort::async_write_to(
                                        &mut writer,
                                        SetWaitPort {
                                            port: my_port,
                                            use_obfuscation: false, // Leave this until the actual logic for it is implemented
                                            obfuscated_port: 0,
                                        },
                                    )
                                    .await,
                                );
                                let _ = block_on(
                                    RoomList::async_write_to(&mut writer, _SendRoomList {}).await,
                                );
                            }
                        }
                        SLSKEvents::RoomList { .. } => (),
                        SLSKEvents::JoinRoom { room, private } => {
                            let _ = block_on(
                                JoinRoom::async_write_to(
                                    &mut writer,
                                    _SendJoinRoom { room, private },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::UpdateRoom { .. } => (),
                        SLSKEvents::ChatroomMessage {
                            room,
                            username,
                            message,
                        } => match username {
                            Some(_) => (),
                            None => {
                                let _ = block_on(
                                    SayChatroom::async_write_to(
                                        &mut writer,
                                        _SendSayChatroom { room, message },
                                    )
                                    .await,
                                );
                            }
                        },
                    },
                    Err(_) => {
                        *quit_w.lock().await = true;
                        let _ = writer.shutdown();
                        return SLSKExitCode::Ok;
                    }
                };
            }
        }
    });

    let read_result = read_task.await;
    match read_result {
        Ok(exit) => match exit {
            SLSKExitCode::LoginFail => {
                write_task.abort();
                return SLSKExitCode::LoginFail;
            }
            _ => (),
        },
        Err(_) => (),
    }

    let write_result = write_task.await;
    match write_result {
        Ok(exit) => exit,
        Err(e) => SLSKExitCode::JoinError(e),
    }
}
