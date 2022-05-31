use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio_util::codec::{FramedRead, LinesCodec};
use std::net::SocketAddr;
use std::env;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
enum Message {
    FromMe(String),
    ToMe(String, String)
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TagchatApp {
    name: String,
    write_msg: String,
    all_messages: Vec<Message>,
    shown_messages: Vec<Message>,
    search_pattern: String,

    #[serde(skip)]
    send: Sender<Message>,
    // #[serde(skip)]
    // recv: Receiver<Message>
}

impl Default for TagchatApp {
    fn default() -> Self {
        let (send, recv) = channel(1024);
        Self {
            name: Default::default(), 
            write_msg: Default::default(), 
            all_messages: Default::default(), 
            shown_messages: Default::default(), 
            search_pattern: Default::default(), 
            
            send, 
        }
    }
}

impl TagchatApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        let (send, mut recv) = channel(1024);

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let mut args = env::args().skip(1).collect::<Vec<_>>();
        let addr = args
            .first()
            .ok_or("this program requires at least one argument").unwrap();
        let addr = addr.parse::<SocketAddr>().unwrap();

        let name = "Szymon";

        std::thread::spawn(move || {
            rt.block_on(async move {
                let stream = TcpStream::connect(addr).await;
                let mut stream = stream.unwrap(); 
                let (mut read, mut write) = tokio::io::split(stream);
                let sent = write.write((name.to_owned() + "\r\n").as_bytes()).await.unwrap();
                println!("SENT {} BYTES", sent);

                tokio::spawn(async move {
                    while let Some(message) = recv.recv().await {
                        println!("RECEIVED MESSAGE\n");
                        match message {
                            Message::FromMe(conent) => {
                                let n_sent = write.write((conent + "\r\n").as_bytes()).await.unwrap();
                                println!("SENT {} BYTES", n_sent);                        
                            },
                            _ => {
                                panic!("Tried to send wrong message.");
                            }
                        }
                    }
                }).await.unwrap();

                // wykomentowanie sprawia ze moveujemy send 


                // let ref mut buffer = vec![0; 1024];
                // tokio::spawn(async move {
                //     loop {
                //         let n_read = read.read(buffer).await.unwrap();
                //         if let Ok(raw_message) = String::from_utf8(buffer[..n_read].to_vec()) {
                //             if let Some((sender, content)) = raw_message.split_once(':') {
                //                 send.send(Message::ToMe(sender.to_string(), content.to_string()));
                //             }
                //         }
                //     }
                // });
            });
        });

        // let framed = Framed::new(stream, LinesCodec::new_with_max_length(1024));
        
        Self {
            name: name.to_owned(),
            write_msg: "".to_owned(),
            all_messages: Vec::new(),
            shown_messages: Vec::new(),
            search_pattern: "".to_owned(),

            send,
        }
    }
}

impl eframe::App for TagchatApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // let Self { label, value } = self;
        let Self {
            name,
            write_msg,
            all_messages,
            shown_messages,
            search_pattern,
            send,
        } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
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
                    send.blocking_send(new_message.clone());
                    all_messages.push(new_message.clone());
                    shown_messages.push(new_message);
                    write_msg.clear();
                }

                // if ui.button("Clear").clicked() {
                //     all_messages.clear();
                //     shown_messages.clear();
                // }
            });
        });
        
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
            // sa = sa.always_show_scroll(true);
            sa = sa.max_height(f32::INFINITY);
            sa.show(ui, |ui| {
                for m in all_messages.clone() {
                    let (sender, content, align) = match m {
                        Message::FromMe(content) => (self.name.clone(), content, egui::Align::RIGHT),
                        Message::ToMe(sender, content) => (sender, content, egui::Align::LEFT)
                    };
                    let s = sender.clone() + ": " + &content;
                    ui.with_layout(
                        egui::Layout::top_down(align),
                        |ui| {
                            ui.label(egui::RichText::new(s).size(23.0));
                        },
                    );
                }
            });
        });

        // if false {
        //     egui::Window::new("Window").show(ctx, |ui| {
        //         ui.label("Windows can be moved by dragging them.");
        //         ui.label("They are automatically sized based on contents.");
        //         ui.label("You can turn on resizing and scrolling if you like.");
        //         ui.label("You would normally chose either panels OR windows.");
        //     });
        // }
    }
}