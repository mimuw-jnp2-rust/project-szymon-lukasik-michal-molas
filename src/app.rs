use clap::Parser;
use futures::future::join_all;
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::mpsc::{channel, Receiver, Sender};

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
enum Message {
    FromMe(String),
    ToMe(String, String),
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
    messages: Vec<(String, String)>,
}

impl SerializedState {
    fn get_messages(&self, name: String) -> Vec<Message> {
        self.messages.iter().map(|x| match &x.0 {
            _ if x.0.eq(&name) => {
                Message::FromMe(x.1.clone())
            },
            _ => {
                Message::ToMe(x.0.clone(), x.1.clone())
            }
        }).collect()
    }

    fn add_message(&mut self, mess: Message, name: String) {
        self.messages.push(match mess {
            Message::FromMe(content) => (name, content),
            Message::ToMe(sender_name, content) => (sender_name, content),
        });
    }
}

impl Default for SerializedState {
    fn default() -> Self {
        Self {
            messages: Default::default(),
        }
    }
}

pub struct TagchatApp {
    state: SerializedState,

    name: String,
    write_msg: String,
    all_messages: Vec<Message>,
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
            all_messages: Default::default(),
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

        let old_messeges: Vec<Message>;
        let prev_state: SerializedState;
        if let Some(storage) = _cc.storage {
            prev_state = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            old_messeges = prev_state.get_messages(name.clone())
        } else {
            prev_state = Default::default();
            old_messeges = Vec::new();
        }

        std::thread::spawn(move || {
            rt.block_on(async move {
                let stream = TcpStream::connect(addr).await;
                let stream = stream.unwrap();
                let (mut read, mut write) = tokio::io::split(stream);
                write
                    .write_all((name + "\r\n").as_bytes())
                    .await
                    .unwrap();

                let write_to_server = tokio::spawn(async move {
                    while let Some(message) = recv.recv().await {
                        match message {
                            Message::FromMe(conent) => {
                                write.write_all((conent + "\r\n").as_bytes()).await.unwrap();
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
                            if let Some((sender, content)) = raw_message.split_once(':') {
                                send.send(Message::ToMe(sender.to_string(), content.to_string()))
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
            all_messages: old_messeges.clone(),
            shown_messages: old_messeges,
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
            all_messages,
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
                    let new_message = Message::FromMe(write_msg.to_string());
                    send.blocking_send(new_message.clone()).unwrap_or_default();
                    all_messages.push(new_message.clone());
                    state.add_message(new_message.clone(), name.clone());
                    shown_messages.push(new_message);
                    write_msg.clear();
                }
            });
        });

        if let Ok(message) = recv.try_recv() {
            all_messages.push(message.clone());
            state.add_message(message.clone(), "".to_string());
            shown_messages.push(message);
        }

        // searching
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search: ");
                let response = ui.text_edit_singleline(search_pattern);
                if response.changed() {
                    shown_messages.clear();
                    for ref m in all_messages.clone() {
                        let m_str = match m {
                            Message::FromMe(content) => content,
                            Message::ToMe(_, content) => content,
                        };

                        if m_str.contains(search_pattern.as_str()) {
                            shown_messages.push(m.clone());
                        }
                    }
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
                        Message::FromMe(content) => (name.clone(), content, egui::Align::RIGHT),
                        Message::ToMe(sender, content) => (sender, content, egui::Align::LEFT),
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
