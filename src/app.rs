use clap::Parser;
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
    messages: Vec<Message>,
    tags: Vec<Tag>,
}

impl Default for SerializedState {
    fn default() -> Self {
        let s = Self {
            messages: Default::default(),
            tags: vec![Default::default()],
        };
        return s;
    }
}

pub struct TagchatApp {
    state: SerializedState,

    name: String,
    write_msg: String,
    shown_messages: Vec<Message>,
    search_pattern: String,
    current_tag: Tag,

    new_tag_name: String,
    new_tag_color: [f32; 4],
    delete_tag: Option<usize>,
    change_messages_tag: Option<(usize, usize, Option<Tag>)>,

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
            shown_messages: Default::default(),
            search_pattern: Default::default(),
            current_tag: Default::default(),

            new_tag_name: Default::default(),
            new_tag_color: Default::default(),
            delete_tag: None,
            change_messages_tag: None,

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
                    while let Some(Message { content, tag, .. }) = recv.recv().await {
                        let mut msg_vec: Vec<u8> = Vec::new();
                        msg_vec.push(content.len() as u8);
                        msg_vec.extend(content.into_bytes());
                        msg_vec.push(tag.name.len() as u8);
                        msg_vec.extend(tag.name.into_bytes());
                        let rgba = tag.color.to_rgba_unmultiplied();
                        for j in 0..4 {
                            let rgba_string = rgba[j].to_string();
                            println!("{}", rgba_string);
                            let mut len = rgba_string.len() as u8;
                            if len > 4 {
                                msg_vec.push(4);
                            } else {
                                msg_vec.push(len);
                            }
                            msg_vec.extend(rgba_string.into_bytes());
                            while len > 4 {
                                msg_vec.pop();
                                len -= 1;
                            }
                        }
                        let msg_string = String::from_utf8(msg_vec).unwrap();

                        write
                            .write_all((msg_string + "\r\n").as_bytes())
                            .await
                            .unwrap();
                    }
                });

                let mut buffer = vec![0; 1024];
                let read_from_server = tokio::spawn(async move {
                    loop {
                        let n_read = read.read(&mut buffer).await.unwrap();
                        if let Ok(raw_message) = String::from_utf8(buffer[..n_read].to_vec()) {
                            let msg_vec = raw_message.into_bytes();
                            let mut msg_strings = Vec::new();
                            let mut i: usize = 0;
                            for _ in 0..3 {
                                let len = msg_vec[i] as usize;
                                i += 1;
                                let msg_part =
                                    String::from_utf8(Vec::from(&msg_vec[i..i + len])).unwrap();
                                i += len;
                                msg_strings.push(msg_part);
                            }

                            let mut tag_rgba: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
                            for j in 0..4 {
                                let rgba_string_len = msg_vec[i] as usize;
                                i += 1;
                                let rgba_string =
                                    String::from_utf8(Vec::from(&msg_vec[i..i + rgba_string_len]))
                                        .unwrap();
                                i += rgba_string_len;
                                println!("{}", rgba_string.parse::<f32>().unwrap());
                                tag_rgba[j] = rgba_string.parse::<f32>().unwrap();
                            }

                            println!("sadasdsa");

                            send.send(Message {
                                content: msg_strings[1].to_string(),
                                tag: Tag {
                                    name: msg_strings[2].to_string(),
                                    color: egui::Rgba::from_rgba_unmultiplied(
                                        tag_rgba[0],
                                        tag_rgba[1],
                                        tag_rgba[2],
                                        tag_rgba[3],
                                    ),
                                },
                                sender: msg_strings[0].to_string(),
                            })
                            .await
                            .unwrap();
                            context.request_repaint();
                        }
                    }
                });

                join_all(vec![write_to_server, read_from_server]).await;
            });
        });

        Self {
            shown_messages: state.messages.clone(),
            state,
            name: args.name.to_owned(),
            write_msg: "".to_owned(),
            search_pattern: "".to_owned(),
            current_tag: Default::default(),
            new_tag_name: Default::default(),
            new_tag_color: Default::default(),
            delete_tag: None,
            change_messages_tag: None,

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
            shown_messages,
            search_pattern,
            ref mut current_tag,
            ref mut new_tag_name,
            ref mut new_tag_color,
            ref mut delete_tag,
            ref mut change_messages_tag,
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
                    state.messages.push(new_message.clone());
                    shown_messages.push(new_message);
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
            let mut new_tag = true;
            for i in 0..state.tags.len() {
                if state.tags[i].name == message.tag.name {
                    new_tag = false;
                }
            }
            if new_tag {
                state.tags.push(message.tag.clone());
            }
            state.messages.push(message.clone());
            shown_messages.push(message);
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
            let response = ui.text_edit_singleline(search_pattern);
            if response.changed() {
                shown_messages.clear();
                for ref m in state.messages.clone() {
                    if m.content.contains(search_pattern.as_str()) {
                        shown_messages.push(m.clone());
                    }
                }
            }
        });

        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            ui.set_min_width(200.);
            ui.label("Your friends");
        });

        if let Some((i, j, Some(chosen_tag))) = (*change_messages_tag).clone() {
            let _ = &state.messages[i..(j + 1)].iter_mut().for_each(|m| {
                m.tag = chosen_tag.clone();
            });
            *change_messages_tag = None;
        }

        // messages window
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            let mut sa: egui::ScrollArea = egui::ScrollArea::vertical();
            sa.max_height(f32::INFINITY)
                .stick_to_bottom()
                .show(ui, |ui| {
                    for (i, m) in shown_messages.iter_mut().enumerate() {
                        let align = if m.sender.eq(name) {
                            egui::Align::RIGHT
                        } else {
                            egui::Align::LEFT
                        };

                        ui.with_layout(egui::Layout::top_down(align), |ui| {
                            let response = ui.add(
                                egui::Button::new(egui::RichText::new(&m.content).size(23.0))
                                    .stroke(egui::Stroke::new(
                                        if let Some(true) = change_messages_tag
                                            .clone()
                                            .and_then(|(_i, _j, _)| Some(_i <= i && i <= _j))
                                            .as_ref()
                                            .as_ref()
                                        {
                                            6.
                                        } else {
                                            3.
                                        },
                                        m.tag.color,
                                    )),
                            );

                            let popup_id = ui.make_persistent_id("my_unique_id");

                            // let response = response.context_menu(|ui| {
                            //     if let Some((i, j, _)) = change_messages_tag.clone() {
                            //         dbg!("CREATING MENU BUTTON");
                            //         let mut chosen_tag: Tag = Default::default();
                            //         if state.tags.iter().any(
                            //             |tag| ui.radio_value(&mut chosen_tag,  tag.clone(), tag.name.clone()).clicked())
                            //         {
                            //             *change_messages_tag = Some((min(i, j), max(i, j), Some(chosen_tag)));
                            //             ui.close_menu();
                            //             ctx.request_repaint();
                            //         }
                            //     }
                            // });

                            if response.secondary_clicked() {
                                dbg!("DOUBLE CLICKING", change_messages_tag.clone());
                                if let Some(true) = change_messages_tag
                                    .clone()
                                    .and_then(|(_i, _j, _)| Some(i == _i && i == _j))
                                {
                                    *change_messages_tag = None;
                                    dbg!("CLICKED SAME");
                                } else {
                                    if let Some(true) = change_messages_tag
                                        .clone()
                                        .and_then(|(_i, _j, _)| Some(_i == _j))
                                    {
                                        dbg!("CLICKED DIFFERENT");
                                        let point = change_messages_tag.clone().unwrap().0;
                                        *change_messages_tag =
                                            Some((min(i, point), max(i, point), None));
                                    } else {
                                        dbg!("CLICKED FIRST");
                                        *change_messages_tag = Some((i, i, None));
                                    }
                                    ui.memory().toggle_popup(popup_id);
                                }
                            }

                            egui::popup::popup_below_widget(ui, popup_id, &response, |ui| {
                                ui.button("Change tag").context_menu(|ui| {
                                    let (i, j, _) = change_messages_tag.clone().unwrap();
                                    dbg!("CREATING MENU BUTTON");
                                    let mut chosen_tag: Tag = Default::default();
                                    if state.tags.iter().any(|tag| {
                                        ui.radio_value(
                                            &mut chosen_tag,
                                            tag.clone(),
                                            tag.name.clone(),
                                        )
                                        .clicked()
                                    }) {
                                        *change_messages_tag =
                                            Some((min(i, j), max(i, j), Some(chosen_tag)));
                                        ui.close_menu();
                                    }
                                });
                            });
                        });

                        // ui.add(egui::Label::new(egui::RichText::new(&m.sender).size(10.0)));
                        ui.add_space(30.);
                    }
                });
        });
    }
}
