use chrono::{DateTime, Utc, Duration};
use eframe::egui::{self, Color32};
use egui_plot::{Line, Plot, PlotPoints};
use reqwest;
use serde::Deserialize;
use rodio::{OutputStream, Sink, Source};
use std::time::Duration as StdDuration;
use std::path::Path;
use egui::Image;
use std::sync::mpsc;

#[derive(Debug, Deserialize)]
struct MarketChart {
    prices: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
struct DailyPrice {
    date: String,
    price: f64,
}

struct AnimatedImage {
    scale: f32,
    target_scale: f32,
    opacity: f32,
    target_opacity: f32,
    float_offset: f32,
    float_time: f32,
}

impl AnimatedImage {
    fn new() -> Self {
        Self {
            scale: 0.8,
            target_scale: 1.0,
            opacity: 0.0,
            target_opacity: 1.0,
            float_offset: 0.0,
            float_time: 0.0,
        }
    }

    fn animate(&mut self, dt: f32) {
        const ANIMATION_SPEED: f32 = 8.0;
        const FLOAT_SPEED: f32 = 2.0;
        const FLOAT_AMPLITUDE: f32 = 10.0;

        self.scale += (self.target_scale - self.scale) * dt * ANIMATION_SPEED;
        self.opacity += (self.target_opacity - self.opacity) * dt * ANIMATION_SPEED;
        
        self.float_time += dt * FLOAT_SPEED;
        self.float_offset = FLOAT_AMPLITUDE * self.float_time.sin();
    }
}

#[derive(Clone)]
struct ChartData {
    daily_prices: Vec<DailyPrice>,
}

struct ImageSequencer {
    bull_index: usize,
    bear_index: usize,
}

impl ImageSequencer {
    fn new() -> Self {
        Self {
            bull_index: 0,
            bear_index: 0,
        }
    }

    fn get_next_bull_index(&mut self) -> usize {
        let index = self.bull_index;
        self.bull_index = (self.bull_index + 1) % 7;  // 7 images de bull
        index
    }

    fn get_next_bear_index(&mut self) -> usize {
        let index = self.bear_index;
        self.bear_index = (self.bear_index + 1) % 4;  // 4 images de bear
        index
    }
}

struct ChartApp {
    daily_prices: Vec<DailyPrice>,
    current_index: usize,
    sound_output: Option<(OutputStream, Sink)>,
    animation_timer: f64,
    bull_textures: Vec<Option<egui::TextureHandle>>,
    bear_textures: Vec<Option<egui::TextureHandle>>,
    current_texture_index: usize,
    image_animation: AnimatedImage,
    point_progress: f32,
    should_return_home: bool,
    image_sequencer: ImageSequencer,
}

impl ChartApp {
    fn new_from_data(data: ChartData) -> Result<Self, Box<dyn std::error::Error>> {
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        
        Ok(Self {
            daily_prices: data.daily_prices,
            current_index: 0,
            sound_output: Some((_stream, sink)),
            animation_timer: 0.0,
            bull_textures: vec![None; 7],
            bear_textures: vec![None; 4],
            current_texture_index: 0,
            image_animation: AnimatedImage::new(),
            point_progress: 0.0,
            should_return_home: false,
            image_sequencer: ImageSequencer::new(),
        })
    }

    fn fetch_data(coin: &str) -> Result<ChartData, Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        
        rt.block_on(async {
            let client = reqwest::Client::new();
            let end = Utc::now();
            let start = end - Duration::days(30);
            
            let url = format!(
                "https://api.coingecko.com/api/v3/coins/{}/market_chart/range?vs_currency=usd&from={}&to={}",
                coin,
                start.timestamp(),
                end.timestamp()
            );

            let response = client.get(&url)
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await?
                .json::<MarketChart>()
                .await?;

            let mut daily_prices = Vec::new();
            let mut last_date = None;

            for (timestamp, price) in response.prices {
                let date = DateTime::<Utc>::from_timestamp((timestamp / 1000.0) as i64, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string();

                if last_date != Some(date.clone()) {
                    daily_prices.push(DailyPrice { 
                        date: date.clone(), 
                        price,
                    });
                    last_date = Some(date);
                }
            }

            Ok(ChartData { daily_prices })
        })
    }

    fn generate_sound(price_change: f64) -> impl Source<Item = f32> + Send {
        let base_freq = 440.0f32;
        let freq = if price_change > 0.0 {
            base_freq / (1.0 + (price_change.abs() / 2.0) as f32)
        } else {
            base_freq * (1.0 + (price_change.abs() / 2.0) as f32)
        };

        rodio::source::SineWave::new(freq)
            .take_duration(StdDuration::from_millis(2000))
            .amplify(0.20)
    }

    fn load_image_if_needed(&mut self, ctx: &egui::Context) {
        // Charger les images de taureaux (bull1 à bull7)
        for i in 0..7 {
            if self.bull_textures[i].is_none() {
                let filename = format!("bull{}.png", i + 1);
                
                match image::io::Reader::open(Path::new("assets").join(&filename)) {
                    Ok(_image_reader) => {
                        self.bull_textures[i] = Some(load_image_from_path(
                            Path::new("assets").join(filename).as_path(),
                            ctx,
                            [400.0, 400.0],
                        ));
                    },
                    Err(e) => {
                        println!("Impossible de charger l'image {}: {}", filename, e);
                    }
                }
            }
        }

        // Charger les images d'ours (bear1 à bear4)
        for i in 0..4 {
            if self.bear_textures[i].is_none() {
                let filename = format!("bear{}.png", i + 1);
                
                match image::io::Reader::open(Path::new("assets").join(&filename)) {
                    Ok(_image_reader) => {
                        self.bear_textures[i] = Some(load_image_from_path(
                            Path::new("assets").join(filename).as_path(),
                            ctx,
                            [400.0, 400.0],
                        ));
                    },
                    Err(e) => {
                        println!("Impossible de charger l'image {}: {}", filename, e);
                    }
                }
            }
        }
    }
}

impl eframe::App for ChartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_image_if_needed(ctx);
        let dt = ctx.input(|i| i.predicted_dt) as f32;
        self.image_animation.animate(dt);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().visuals.extreme_bg_color = Color32::from_rgb(18, 18, 18);
            ui.style_mut().visuals.panel_fill = Color32::from_rgb(24, 24, 24);

            // Ajouter le bouton en haut à gauche
            ui.horizontal(|ui| {
                if ui.button(
                    egui::RichText::new("← Back to Home")
                        .size(16.0)
                        .color(Color32::from_rgb(255, 215, 0))
                ).clicked() {
                    // On utilisera cette information dans MainApp
                    self.should_return_home = true;
                }
                ui.add_space(ui.available_width());  // Pour pousser le bouton à gauche
            });

            let current_data: Vec<[f64; 2]> = self.daily_prices[..=self.current_index.min(self.daily_prices.len()-1)]
                .iter()
                .enumerate()
                .map(|(day, price_data)| {
                    let day = day as f64 * 2.0;
                    let price = price_data.price;
                    [day, price]
                })
                .collect();

            let mut green_segments = Vec::new();
            let mut red_segments = Vec::new();
            
            for window in current_data.windows(2) {
                let [_day1, price1] = window[0];
                let [_day2, price2] = window[1];
                if price1 <= price2 {
                    green_segments.push(window.to_vec());
                } else {
                    red_segments.push(window.to_vec());
                }
            }

            let prices_clone = self.daily_prices.clone();
            let prices_clone2 = prices_clone.clone();
            Plot::new("Ethereum Price")
                .height(ui.available_height())
                .width(ui.available_width())
                .include_y(0.0)
                .include_x(-2.0)
                .include_x((self.daily_prices.len() as f64) * 2.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .label_formatter(move |_name, value| {
                    let day_index = (value.x / 2.0) as usize;
                    if day_index >= prices_clone.len() {
                        return String::new();
                    }
                    let date = &prices_clone[day_index].date;
                    let formatted_date = format!("{}/{}", &date[8..10], &date[5..7]);
                    format!(
                        "day {}\nprice(usd) = {:.1}",
                        formatted_date,
                        value.y
                    )
                })
                .x_axis_formatter(move |x, _range, _precision| {
                    let day_index = (x.value / 2.0) as usize;
                    if day_index >= prices_clone2.len() {
                        return String::new();
                    }
                    let date = &prices_clone2[day_index].date;
                    format!("{}/{}", &date[8..10], &date[5..7])
                })
                .show(ui, |plot_ui| {
                    for segment in green_segments {
                        plot_ui.line(Line::new(PlotPoints::new(segment))
                            .color(Color32::from_rgb(46, 189, 89))
                            .width(1.5));
                    }
                    for segment in red_segments {
                        plot_ui.line(Line::new(PlotPoints::new(segment))
                            .color(Color32::from_rgb(255, 88, 88))
                            .width(1.5));
                    }

                    plot_ui.points(egui_plot::Points::new(PlotPoints::new(current_data))
                        .color(Color32::from_rgb(255, 255, 255))
                        .radius(0.5)
                        .filled(true));
                });

            if self.current_index > 0 {
                let current_price = self.daily_prices[self.current_index].price;
                let previous_price = self.daily_prices[self.current_index - 1].price;
                let is_bullish = current_price >= previous_price;

                let base_size = 400.0;
                let scaled_size = base_size * self.image_animation.scale;
                let image_size = [scaled_size, scaled_size];
                
                let screen_rect = ui.max_rect();
                let image_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        screen_rect.right() - image_size[0] - 20.0,
                        screen_rect.bottom() - image_size[1] - 20.0 + self.image_animation.float_offset,
                    ),
                    image_size.into(),
                );

                let texture = if is_bullish {
                    self.bull_textures[self.current_texture_index].as_ref().unwrap()
                } else {
                    self.bear_textures[self.current_texture_index].as_ref().unwrap()
                };

                let image = Image::new(texture)
                    .tint(Color32::from_white_alpha((255.0 * self.image_animation.opacity) as u8));
                ui.put(image_rect, image);
            }
        });

        // Animation des points
        self.point_progress += dt * 2.0;
        if self.point_progress > 1.0 {
            self.point_progress = 1.0;
        }

        self.animation_timer += dt as f64;
        
        if let Some((_, sink)) = &self.sound_output {
            if !sink.empty() {
                ctx.request_repaint();
                return;
            }
        }

        if self.animation_timer >= 2.0 && self.current_index < self.daily_prices.len() - 1 {
            let current_price = self.daily_prices[self.current_index].price;
            let next_price = self.daily_prices[self.current_index + 1].price;
            let price_change = ((next_price - current_price) / current_price) * 100.0;

            if let Some((_, sink)) = &self.sound_output {
                sink.append(Self::generate_sound(price_change));
            }

            // Reset des animations
            self.image_animation.scale = 0.8;
            self.image_animation.opacity = 0.0;
            self.point_progress = 0.0;

            self.current_index += 1;
            self.animation_timer = 0.0;
            
            // Utiliser le sequencer pour obtenir le prochain index
            self.current_texture_index = if current_price < next_price {
                self.image_sequencer.get_next_bull_index()
            } else {
                self.image_sequencer.get_next_bear_index()
            };
        }

        ctx.request_repaint();
    }
}

// Fonction utilitaire pour charger les images
fn load_image_from_path(path: &Path, ctx: &egui::Context, size: [f32; 2]) -> egui::TextureHandle {
    let image = image::io::Reader::open(path)
        .unwrap()
        .decode()
        .unwrap()
        .resize(
            size[0] as u32,
            size[1] as u32,
            image::imageops::FilterType::Triangle,
        );
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    
    ctx.load_texture(
        path.file_name().unwrap().to_str().unwrap(),
        egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
        egui::TextureOptions::default(),
    )
}

enum Page {
    Selection,
    EthChart,
    BtcChart,
    XrpChart,
}

struct SelectionPage {
    vitalik_texture: Option<egui::TextureHandle>,
    satoshi_texture: Option<egui::TextureHandle>,
    david_texture: Option<egui::TextureHandle>,
}

impl SelectionPage {
    fn new() -> Self {
        Self {
            vitalik_texture: None,
            satoshi_texture: None,
            david_texture: None,
        }
    }

    fn load_images_if_needed(&mut self, ctx: &egui::Context) {
        if self.vitalik_texture.is_none() {
            let path = Path::new("assets").join("vitalik.png");
            self.vitalik_texture = Some(load_image_from_path(
                &path,
                ctx,
                [300.0, 300.0],
            ));
        }
        if self.satoshi_texture.is_none() {
            let path = Path::new("assets").join("satoshi.png");
            self.satoshi_texture = Some(load_image_from_path(
                &path,
                ctx,
                [300.0, 300.0],
            ));
        }
        if self.david_texture.is_none() {
            let path = Path::new("assets").join("david_xrp.png");
            self.david_texture = Some(load_image_from_path(
                &path,
                ctx,
                [300.0, 300.0],
            ));
        }
    }
}

#[derive(PartialEq)]
enum LoadingState {
    NotLoading,
    Loading(String),
}

struct MainApp {
    current_page: Page,
    selection_page: SelectionPage,
    eth_chart: Option<ChartApp>,
    btc_chart: Option<ChartApp>,
    xrp_chart: Option<ChartApp>,
    loading_state: LoadingState,
    data_receiver: Option<mpsc::Receiver<(String, ChartData)>>,
}

impl MainApp {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            current_page: Page::Selection,
            selection_page: SelectionPage::new(),
            eth_chart: None,
            btc_chart: None,
            xrp_chart: None,
            loading_state: LoadingState::NotLoading,
            data_receiver: None,
        })
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(receiver) = &self.data_receiver {
            if let Ok((coin, data)) = receiver.try_recv() {
                if let Ok(chart) = ChartApp::new_from_data(data) {
                    match coin.as_str() {
                        "ethereum" => {
                            self.eth_chart = Some(chart);
                            self.current_page = Page::EthChart;
                        },
                        "bitcoin" => {
                            self.btc_chart = Some(chart);
                            self.current_page = Page::BtcChart;
                        },
                        "ripple" => {
                            self.xrp_chart = Some(chart);
                            self.current_page = Page::XrpChart;
                        },
                        _ => {}
                    }
                    self.loading_state = LoadingState::NotLoading;
                    self.data_receiver = None;
                }
            }
        }

        match self.current_page {
            Page::Selection => {
                self.selection_page.load_images_if_needed(ctx);
                
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Fond sombre
                    ui.style_mut().visuals.extreme_bg_color = Color32::from_rgb(18, 18, 18);
                    ui.style_mut().visuals.panel_fill = Color32::from_rgb(24, 24, 24);

                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        
                        // Titre principal
                        ui.heading(egui::RichText::new("Crypto Price Sonifier")
                            .size(40.0)
                            .color(Color32::from_rgb(255, 215, 0)));
                        
                        ui.add_space(20.0);
                        
                        // Description de l'application
                        ui.label(egui::RichText::new(
                            "Experience cryptocurrency price movements through sound and visuals.\n\
                            Watch and listen as the market evolves over the last 30 days."
                        ).size(16.0)
                        .color(Color32::LIGHT_GRAY));
                        
                        ui.add_space(40.0);
                        
                        // Sous-titre
                        ui.heading(egui::RichText::new("Choose Your Side")
                            .size(24.0)
                            .color(Color32::WHITE));
                        
                        ui.add_space(30.0);

                        // Images côte à côte avec descriptions
                        let center_x = ui.available_width() / 2.0;
                        let btc_x = center_x - 125.0;  // 250/2 pour centrer Bitcoin
                        let eth_x = btc_x - 300.0;     // 250 + 50 (espace) pour Ethereum
                        let xrp_x = btc_x + 300.0;     // 250 + 50 (espace) pour XRP

                        ui.horizontal(|ui| {
                            // Ethereum (à gauche)
                            ui.allocate_ui_at_rect(
                                egui::Rect::from_min_size(
                                    egui::pos2(eth_x, ui.min_rect().top()),
                                    egui::vec2(250.0, 300.0)
                                ),
                                |ui| {
                                    ui.vertical_centered(|ui| {
                                        let vitalik_image = Image::new(
                                            self.selection_page.vitalik_texture.as_ref().unwrap()
                                        )
                                        .fit_to_exact_size([250.0, 250.0].into())
                                        .rounding(8.0);
                                        
                                        if ui.add(egui::ImageButton::new(vitalik_image)
                                            .frame(true)
                                            .selected(false)
                                        ).clicked() {
                                            self.loading_state = LoadingState::Loading("Ethereum".to_string());
                                            let (tx, rx) = mpsc::channel();
                                            self.data_receiver = Some(rx);
                                            let ctx = ctx.clone();
                                            
                                            std::thread::spawn(move || {
                                                if let Ok(data) = ChartApp::fetch_data("ethereum") {
                                                    tx.send(("ethereum".to_string(), data)).ok();
                                                    ctx.request_repaint();
                                                }
                                            });
                                        }
                                        
                                        ui.add_space(10.0);
                                        ui.colored_label(
                                            Color32::from_rgb(114, 137, 218),
                                            egui::RichText::new("Ethereum (ETH)")
                                                .size(24.0)
                                                .strong()
                                        );
                                        ui.label(
                                            egui::RichText::new("Smart contracts pioneer")
                                                .size(16.0)
                                                .color(Color32::LIGHT_GRAY)
                                        );
                                    });
                                }
                            );

                            // Bitcoin (au centre)
                            ui.allocate_ui_at_rect(
                                egui::Rect::from_min_size(
                                    egui::pos2(btc_x, ui.min_rect().top()),
                                    egui::vec2(250.0, 300.0)
                                ),
                                |ui| {
                                    ui.vertical_centered(|ui| {
                                        let satoshi_image = Image::new(
                                            self.selection_page.satoshi_texture.as_ref().unwrap()
                                        )
                                        .fit_to_exact_size([250.0, 250.0].into())
                                        .rounding(8.0);
                                        
                                        if ui.add(egui::ImageButton::new(satoshi_image)
                                            .frame(true)
                                            .selected(false)
                                        ).clicked() {
                                            self.loading_state = LoadingState::Loading("Bitcoin".to_string());
                                            let (tx, rx) = mpsc::channel();
                                            self.data_receiver = Some(rx);
                                            let ctx = ctx.clone();
                                            
                                            std::thread::spawn(move || {
                                                if let Ok(data) = ChartApp::fetch_data("bitcoin") {
                                                    tx.send(("bitcoin".to_string(), data)).ok();
                                                    ctx.request_repaint();
                                                }
                                            });
                                        }
                                        
                                        ui.add_space(10.0);
                                        ui.colored_label(
                                            Color32::from_rgb(247, 147, 26),
                                            egui::RichText::new("Bitcoin (BTC)")
                                                .size(24.0)
                                                .strong()
                                        );
                                        ui.label(
                                            egui::RichText::new("Digital gold & store of value")
                                                .size(16.0)
                                                .color(Color32::LIGHT_GRAY)
                                        );
                                    });
                                }
                            );

                            // XRP (à droite)
                            ui.allocate_ui_at_rect(
                                egui::Rect::from_min_size(
                                    egui::pos2(xrp_x, ui.min_rect().top()),
                                    egui::vec2(250.0, 300.0)
                                ),
                                |ui| {
                                    ui.vertical_centered(|ui| {
                                        let david_image = Image::new(
                                            self.selection_page.david_texture.as_ref().unwrap()
                                        )
                                        .fit_to_exact_size([250.0, 250.0].into())
                                        .rounding(8.0);
                                        
                                        if ui.add(egui::ImageButton::new(david_image)
                                            .frame(true)
                                            .selected(false)
                                        ).clicked() {
                                            self.loading_state = LoadingState::Loading("Ripple".to_string());
                                            let (tx, rx) = mpsc::channel();
                                            self.data_receiver = Some(rx);
                                            let ctx = ctx.clone();
                                            
                                            std::thread::spawn(move || {
                                                if let Ok(data) = ChartApp::fetch_data("ripple") {
                                                    tx.send(("ripple".to_string(), data)).ok();
                                                    ctx.request_repaint();
                                                }
                                            });
                                        }
                                        
                                        ui.add_space(10.0);
                                        ui.colored_label(
                                            Color32::from_rgb(0, 153, 204),
                                            egui::RichText::new("Ripple (XRP)")
                                                .size(24.0)
                                                .strong()
                                        );
                                        ui.label(
                                            egui::RichText::new("Global payments solution")
                                                .size(16.0)
                                                .color(Color32::LIGHT_GRAY)
                                        );
                                    });
                                }
                            );
                        });

                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("Click on an icon to start the price sonification")
                                .size(14.0)
                                .italics()
                                .color(Color32::GRAY)
                        );
                    });

                    // Afficher l'overlay de chargement si nécessaire
                    if let LoadingState::Loading(crypto_name) = &self.loading_state {
                        let screen_rect = ui.max_rect();
                        
                        // Overlay sombre semi-transparent
                        ui.painter().rect_filled(
                            screen_rect,
                            0.0,
                            Color32::from_black_alpha(192)
                        );

                        // Message de chargement avec animation
                        let time = ui.input(|i| i.time);
                        let dots = ".".repeat((time * 2.0) as usize % 4);
                        let loading_text = format!("Fetching {} price data{}", crypto_name, dots);

                        // Centrer le texte
                        let text_size = egui::Vec2::new(400.0, 50.0);
                        let text_rect = egui::Rect::from_center_size(
                            screen_rect.center(),
                            text_size,
                        );

                        // Afficher le texte centré
                        ui.put(text_rect, egui::Label::new(
                            egui::RichText::new(loading_text)
                                .size(24.0)
                                .color(Color32::WHITE)
                                .text_style(egui::TextStyle::Heading)
                        ));

                        ctx.request_repaint();  // Pour l'animation des points
                    }
                });
            },
            Page::EthChart => {
                if let Some(chart) = &mut self.eth_chart {
                    chart.update(ctx, frame);
                    if chart.should_return_home {
                        self.current_page = Page::Selection;
                        self.eth_chart = None;
                    }
                }
            },
            Page::BtcChart => {
                if let Some(chart) = &mut self.btc_chart {
                    chart.update(ctx, frame);
                    if chart.should_return_home {
                        self.current_page = Page::Selection;
                        self.btc_chart = None;
                    }
                }
            },
            Page::XrpChart => {
                if let Some(chart) = &mut self.xrp_chart {
                    chart.update(ctx, frame);
                    if chart.should_return_home {
                        self.current_page = Page::Selection;
                        self.xrp_chart = None;
                    }
                }
            },
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 660.0])
            .with_min_inner_size([400.0, 300.0])
            .with_window_level(egui::WindowLevel::Normal)
            .with_decorations(true)
            .with_transparent(false),
        default_theme: eframe::Theme::Dark,
        follow_system_theme: false,
        vsync: true,
        multisampling: 4,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };

    eframe::run_native(
        "Crypto Chart",
        options,
        Box::new(|_cc| Box::new(MainApp::new().unwrap())),
    ).unwrap();

    Ok(())
}