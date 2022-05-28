/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TagchatApp {
    name: String,
    write_msg: String,
    all_messages: Vec<(String, String)>,
    shown_messages: Vec<(String, String)>,

    search_pattern: String,
    // this how you opt-out of serialization of a member
    // #[serde(skip)]
    // value: f32,
}

impl Default for TagchatApp {
    fn default() -> Self {
        Self {
            name: "Michal".to_owned(),
            write_msg: "".to_owned(),
            all_messages: Vec::new(),
            shown_messages: Vec::new(),
            search_pattern: "".to_owned(),
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

        Default::default()
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
                    all_messages.push((name.to_string(), write_msg.to_string()));
                    write_msg.clear();
                    shown_messages.clear();
                    let all_m = &mut all_messages.clone();
                    shown_messages.append(all_m);
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
                    for m in all_messages {
                        let m_str = m.1.as_str();
                        if m_str.contains(search_pattern.as_str()) {
                            shown_messages.push((m.0.to_string(), m_str.to_string()));
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
                for m in shown_messages.clone() {
                    let mut s = m.0.to_string();
                    s.push_str(": ");
                    s.push_str(m.1.as_str());
                    ui.heading(s);
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
