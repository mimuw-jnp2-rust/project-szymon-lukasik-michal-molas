use clap::Parser;
use egui::CollapsingHeader;
use futures::future::join_all;
use std::cmp::{max, min};
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::mpsc::{channel, Receiver, Sender};

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
struct Tag {
    name: String,
    color: egui::color::Rgba,
}

impl Default for Tag {
    fn default() -> Self {
        Tag {
            name: "undefined".into(),
            color: egui::Rgba::BLACK,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct Message {
    content: String,
    tag: Tag,
    sender: String,
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

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[serde(default)]
struct SerializedState {
    rooms: Vec<Vec<Message>>,
    // messages: Vec<Message>,
    tags: Vec<Tag>,
}

impl Default for SerializedState {
    fn default() -> Self {
        let s = Self {
            rooms: vec![Default::default()],
            // messages: Default::default(),
            tags: vec![Default::default()],
        };
        return s;
    }
}

pub struct TagchatApp {
    state: SerializedState,

    name: String,
    write_msg: String,
    search_pattern: String,
    current_tag: Tag,
    current_room: usize,

    new_tag_name: String,
    new_tag_color: [f32; 4],
    delete_tag: Option<usize>,
    marked_messages: Option<(usize, usize)>,

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
            search_pattern: Default::default(),
            current_tag: Default::default(),
            current_room: Default::default(),

            new_tag_name: Default::default(),
            new_tag_color: Default::default(),
            delete_tag: None,
            marked_messages: None,

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

        let state: SerializedState = _cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        let context = _cc.egui_ctx.clone();
        std::thread::spawn(move || {
            rt.block_on(async move {
                let stream = TcpStream::connect(addr).await;
                let stream = stream.unwrap();
                let (mut read, mut write) = tokio::io::split(stream);
                write.write_all((name + "\r\n").as_bytes()).await.unwrap();

                let write_to_server = tokio::spawn(async move {
                    while let Some(Message { content, .. }) = recv.recv().await {
                        let s = content + "\r\n";
                        write.write_all((s).as_bytes()).await.unwrap();
                    }
                });

                let mut buffer = vec![0; 1024];
                let read_from_server = tokio::spawn(async move {
                    loop {
                        let n_read = read.read(&mut buffer).await.unwrap();
                        if let Ok(raw_message) = String::from_utf8(buffer[..n_read].to_vec()) {
                            if let Some((sender, content)) = raw_message.split_once(':') {
                                send.send(Message {
                                    content: content.to_string(),
                                    tag: Default::default(),
                                    sender: sender.to_string(),
                                })
                                .await
                                .unwrap();
                                context.request_repaint();
                            }
                        }
                    }
                });

                join_all(vec![write_to_server, read_from_server]).await;
            });
        });

        Self {
            state,
            name: args.name.to_owned(),
            write_msg: "".to_owned(),
            search_pattern: "".to_owned(),
            current_tag: Default::default(),
            current_room: Default::default(),
            new_tag_name: Default::default(),
            new_tag_color: Default::default(),
            delete_tag: None,
            marked_messages: None,

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
            search_pattern,
            ref mut current_tag,
            current_room,
            ref mut new_tag_name,
            ref mut new_tag_color,
            ref mut delete_tag,
            ref mut marked_messages,
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
            ui.horizontal_top(|ui| {
                ui.set_min_height(100.);
                ui.label("Write your message: ");
                ui.add(egui::TextEdit::multiline(write_msg));
                if ui.add(egui::Button::new("Send")).clicked() {
                    let new_message = Message {
                        content: write_msg.to_string(),
                        tag: current_tag.clone(),
                        sender: name.clone(),
                    };
                    send.blocking_send(new_message.clone()).unwrap_or_default();
                    state.rooms[*current_room].push(new_message.clone());
                    write_msg.clear();
                }

                if ui
                    .add(
                        egui::Button::new(current_tag.name.clone())
                            .stroke(egui::Stroke::new(3., current_tag.color)),
                    )
                    .clicked()
                {}

                ui.menu_button("Change tag", |ui| {
                    let sa: egui::ScrollArea = egui::ScrollArea::vertical().max_height(50.);
                    sa.show(ui, |ui| {
                        if state.tags.iter().any(|tag| {
                            ui.radio_value(current_tag, tag.clone(), tag.name.clone())
                                .clicked()
                        }) {
                            ui.close_menu();
                        }
                    });
                });
            });
        });

        if let Ok(message) = recv.try_recv() {
            state.rooms[*current_room].push(message.clone());
        }

        if let Some(tag_idx) = delete_tag {
            state.tags.remove(*tag_idx);
            *delete_tag = None;
        }

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.set_max_width(200.);
            egui::CollapsingHeader::new("Your tags")
                .default_open(false)
                .show(ui, |ui| {
                    for (i, tag) in state.tags.iter().enumerate() {
                        ui.add(
                            egui::Button::new(tag.name.clone())
                                .stroke(egui::Stroke::new(3., tag.color)),
                        )
                        .context_menu(|ui| {
                            if ui.button("Delete").clicked() {
                                *delete_tag = Some(i);
                                ui.close_menu();
                            }
                        });
                    }

                    ui.horizontal(|ui| {
                        ui.label("Add new tag");
                        ui.text_edit_singleline(new_tag_name);
                        ui.color_edit_button_rgba_unmultiplied(new_tag_color);

                        if ui.button("Add").clicked() && !new_tag_name.is_empty() {
                            let [r, g, b, a] = new_tag_color.clone();
                            state.tags.push(Tag {
                                name: new_tag_name.clone(),
                                color: egui::Rgba::from_rgba_unmultiplied(r, g, b, a),
                            });

                            *new_tag_name = Default::default();
                            *new_tag_color = Default::default();
                        }
                    });
                });

            ui.label("Search: ");
            ui.text_edit_singleline(search_pattern);

            egui::ComboBox::from_label("Select room")
                .selected_text(format!("{:?}", current_room))
                .show_ui(ui, |ui| {
                    for i in 0..state.rooms.len() {
                        // let cr = *current_room;
                        if ui
                            .selectable_value(&mut *current_room, i, format!("{:?}", i))
                            .clicked()
                        {}
                    }
                });

            if ui.add(egui::Button::new("Add new room")).clicked() {
                state.rooms.push(Vec::new());
            }
        });

        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            ui.set_min_width(200.);
            ui.label("Your friends");
        });

        // messages window
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            let sa: egui::ScrollArea = egui::ScrollArea::vertical();
            sa.max_height(f32::INFINITY)
                .stick_to_bottom()
                .show(ui, |ui| {
                    for (m_idx, m) in state.rooms[*current_room]
                        .clone()
                        .iter()
                        .filter(|m| m.content.contains(search_pattern.as_str()))
                        .enumerate()
                    {
                        let align = if m.sender.eq(name) {
                            egui::Align::RIGHT
                        } else {
                            egui::Align::LEFT
                        };

                        ui.with_layout(egui::Layout::top_down(align), |ui| {
                            let response = ui.add(
                                egui::Button::new(egui::RichText::new(&m.content).size(23.0))
                                    .stroke(egui::Stroke::new(
                                        if let Some(true) = marked_messages
                                            .clone()
                                            .and_then(|(i, j)| Some(i <= m_idx && m_idx <= j))
                                        {
                                            6.
                                        } else {
                                            3.
                                        },
                                        m.tag.color,
                                    )),
                            );

                            if search_pattern.is_empty() {
                                if response.clicked() {
                                    if let Some(true) = marked_messages
                                        .clone()
                                        .and_then(|(i, j)| Some(m_idx == i && i == j))
                                    {
                                        *marked_messages = None;
                                    } else {
                                        if let Some(true) =
                                            marked_messages.clone().and_then(|(i, j)| Some(i == j))
                                        {
                                            let point = marked_messages.clone().unwrap().0;
                                            *marked_messages =
                                                Some((min(m_idx, point), max(m_idx, point)));
                                        } else {
                                            *marked_messages = Some((m_idx, m_idx));
                                        }
                                    }
                                }

                                if let Some((i, j)) = marked_messages.clone() {
                                    if i <= m_idx && m_idx <= j {
                                        response.context_menu(|ui| {
                                            if ui.button("Delete").clicked() {
                                                ui.close_menu();
                                                state.rooms[*current_room].drain(i..(j + 1));
                                                *marked_messages = None;
                                                ctx.request_repaint();
                                            }

                                            egui::CollapsingHeader::new("Change tag")
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    let mut chosen_tag: Tag = m.tag.clone();
                                                    if state.tags.iter().any(|tag| {
                                                        ui.radio_value(
                                                            &mut chosen_tag,
                                                            tag.clone(),
                                                            tag.name.clone(),
                                                        )
                                                        .clicked()
                                                    }) {
                                                        ui.close_menu();
                                                        let _ = &state.rooms[*current_room]
                                                            [i..(j + 1)]
                                                            .iter_mut()
                                                            .for_each(|m| {
                                                                m.tag = chosen_tag.clone();
                                                            });
                                                        *marked_messages = None;
                                                        ctx.request_repaint();
                                                    }
                                                });
                                        });
                                    }
                                }
                            }
                        });

                        // ui.add(egui::Label::new(egui::RichText::new(&m.sender).size(10.0)));
                        ui.add_space(30.);
                    }
                });
        });
    }
}
