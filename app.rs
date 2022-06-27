use clap::Parser;
use futures::future::join_all;
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::mpsc::{channel, Receiver, Sender};

type Room = Vec<Message>;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
enum Message {
    FromMe(String, u8),
    ToMe(String, String, u8),
}

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, parse(try_from_str = parse_addr))]
    server_addr: SocketAddr,

    #[clap(short, long)]
    name: String,
}

fn parse_addr(s: &str) -> Result<SocketAddr, String> {
    s.to_socket_addrs()
        .map_err(|e| e.to_string())
        .and_then(|mut iter| iter.next().ok_or_else(|| "No address found".to_string()))
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct SerializedState {
    // messages: Vec<(String, String)>,
    rooms: Vec<Vec<(String, String)>>,
}

impl SerializedState {
    fn get_messages(&self, name: String) -> Vec<Room> {
        let mut rooms_ret: Vec<Room> = Vec::new();
        for i in 0..self.rooms.len() {
            rooms_ret.push(
                self.rooms[i]
                    .iter()
                    .map(|x| match &x.0 {
                        _ if x.0.eq(&name) => Message::FromMe(x.1.clone(), i as u8), // co zamiast 0?
                        _ => Message::ToMe(x.0.clone(), x.1.clone(), i as u8),
                    })
                    .collect(),
            )
        }
        rooms_ret
    }

    fn add_message(&mut self, mess: Message, name: String, room_idx: usize) {
        self.rooms[room_idx].push(match mess {
            Message::FromMe(content, _) => (name, content),
            Message::ToMe(sender_name, content, _) => (sender_name, content),
        });
    }

    fn add_room(&mut self) {
        self.rooms.push(Vec::new());
    }
}

impl Default for SerializedState {
    fn default() -> Self {
        Self {
            // messages: Default::default(),
            rooms: vec![Vec::new()],
        }
    }
}

pub struct TagchatApp {
    state: SerializedState,

    name: String,
    write_msg: String,
    // all_messages: Vec<Message>,
    current_room: usize,
    rooms: Vec<Room>,
    shown_messages: Vec<Message>,
    search_pattern: String,

    send: Sender<Message>,
    recv: Receiver<Message>,
}

impl Default for TagchatApp {
    fn default() -> Self {
        let (send, recv) = channel(1024);
        Self {
            state: Default::default(),
            name: Default::default(),
            write_msg: Default::default(),
            current_room: 0,
            rooms: Default::default(),
            // all_messages: Default::default(),
            shown_messages: Default::default(),
            search_pattern: Default::default(),

            send,
            recv,
        }
    }
}

impl TagchatApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (my_send, mut recv) = channel(1024);
        let (send, my_recv) = channel(1024);

        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        let args: Args = Args::parse();

        let name = args.name.clone();
        let addr = args.server_addr.clone();

        // let old_messeges: Vec<Message>;
        let mut old_rooms: Vec<Room>;
        let prev_state: SerializedState;
        if let Some(storage) = _cc.storage {
            prev_state = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            // old_messeges = prev_state.get_messages(name.clone(), 0);
            old_rooms = prev_state.get_messages(name.clone());
        } else {
            prev_state = Default::default();
            // old_messeges = Vec::new();
            old_rooms = Vec::new();
        }
        if old_rooms.is_empty() {
            old_rooms.push(Vec::new());
        }

        std::thread::spawn(move || {
            rt.block_on(async move {
                let stream = TcpStream::connect(addr).await;
                let stream = stream.unwrap();
                let (mut read, mut write) = tokio::io::split(stream);
                write.write_all((name + "\r\n").as_bytes()).await.unwrap();

                let write_to_server = tokio::spawn(async move {
                    while let Some(message) = recv.recv().await {
                        match message {
                            Message::FromMe(content, room_idx) => {
                                write.write_all((content + ":" + &room_idx.to_string() + "\r\n").as_bytes()).await.unwrap();
                            }
                            _ => {
                                panic!("Tried to send wrong message.");
                            }
                        }
                    }
                });

                let mut buffer = vec![0; 1024];
                let read_from_server = tokio::spawn(async move {
                    loop {
                        let n_read = read.read(&mut buffer).await.unwrap();
                        if let Ok(raw_message) = String::from_utf8(buffer[..n_read].to_vec()) {
                            println!("{}", raw_message);
                            let split_message = raw_message.split(':').collect::<Vec<_>>();
                            if split_message.len() == 3 {
                                println!("{}", split_message[2]);
                                send.send(Message::ToMe(split_message[0].to_string(), split_message[1].to_string(), split_message[2][..split_message.len() - 2].parse().expect("Not a number")))
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                });

                join_all(vec![write_to_server, read_from_server]).await;
            });
        });

        Self {
            state: prev_state,
            name: args.name.to_owned(),
            write_msg: "".to_owned(),
            // all_messages: old_messeges.clone(),
            current_room: 0,
            rooms: old_rooms.clone(),
            shown_messages: old_rooms[0].clone(),
            search_pattern: "".to_owned(),

            send: my_send,
            recv: my_recv,
        }
    }
}

impl eframe::App for TagchatApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Self {
            state,
            name,
            write_msg,
            current_room,
            rooms,
            shown_messages,
            search_pattern,
            send,
            recv,
        } = self;

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
                });
            });
        });

        // Writing new msg
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label("Write your message: ");
                let response = ui.text_edit_singleline(write_msg);
                if response.lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                    let new_message = Message::FromMe(write_msg.to_string(), *current_room as u8);
                    send.blocking_send(new_message.clone()).unwrap_or_default();
                    // all_messages.push(new_message.clone());
                    rooms[*current_room].push(new_message.clone());
                    state.add_message(new_message.clone(), name.clone(), *current_room);
                    shown_messages.push(new_message);
                    write_msg.clear();
                }
            });
        });

        if let Ok(message) = recv.try_recv() {
            let idx = match message {
                Message::ToMe(_, _, room_idx) => room_idx, 
                _ => panic!("Bad message"),
            } as usize;

            rooms[idx].push(message.clone());
            state.add_message(message.clone(), "".to_string(), idx);
            if *current_room == idx {
                shown_messages.push(message);
            }
        }

        // searching
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label("Search: ");
                let response = ui.text_edit_singleline(search_pattern);
                if response.changed() {
                    shown_messages.clear();
                    // for ref m in all_messages.clone() {
                    for ref m in rooms[*current_room].clone() {
                        let m_str = match m {
                            Message::FromMe(content, _) => content,
                            Message::ToMe(_, content, _) => content,
                        };

                        if m_str.contains(search_pattern.as_str()) {
                            shown_messages.push(m.clone());
                        }
                    }
                }

                egui::ComboBox::from_label("Select room")
                    .selected_text(format!("{:?}", current_room))
                    .show_ui(ui, |ui| {
                        for i in 0..rooms.len() {
                            // let cr = *current_room;
                            if ui.selectable_value(&mut *current_room, i, format!("{:?}", i)).clicked() {
                                shown_messages.clear();
                                *shown_messages = rooms[*current_room].clone();
                            }
                        }
                        // ui.selectable_value(&mut *current_room, 0, "First");
                        // ui.selectable_value(&mut *current_room, 1, "Second");
                        // ui.selectable_value(&mut *current_room, 2, "Third");
                    });

                if ui.add(egui::Button::new("Add new room")).clicked() {
                    state.add_room();
                    rooms.push(Vec::new());
                }
            });
        });

        // messages window
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            let mut sa: egui::ScrollArea = egui::ScrollArea::vertical();
            sa = sa.max_height(f32::INFINITY);
            sa.show(ui, |ui| {
                for m in shown_messages.clone() {
                    let (sender, content, align) = match m {
                        Message::FromMe(content, _) => (name.clone(), content, egui::Align::RIGHT),
                        Message::ToMe(sender, content, _) => (sender, content, egui::Align::LEFT),
                    };
                    let s = sender.clone() + ": " + &content;
                    ui.with_layout(egui::Layout::top_down(align), |ui| {
                        ui.label(egui::RichText::new(s).size(23.0));
                    });
                }
            });
        });
    }
}
